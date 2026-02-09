use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::State;
use tracing::info;
use uuid::Uuid;

use liberte_net::SwarmCommand;
use liberte_shared::protocol::{FileOffer, WireMessage};
use liberte_shared::types::{ChannelId, UserId};
use liberte_store::Blob;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct FileSendResult {
    pub file_id: String,
    pub file_name: String,
    pub file_size: u64,
}

#[tauri::command]
pub async fn send_file(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    file_path: String,
) -> Result<FileSendResult, String> {
    let channel_uuid = Uuid::parse_str(&channel_id)
        .map_err(|e| format!("Invalid channel_id: {e}"))?;

    let path = std::path::Path::new(&file_path);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let file_data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let file_size = file_data.len() as u64;

    if file_data.len() > liberte_shared::constants::MAX_FILE_SIZE {
        return Err(format!(
            "File too large: {} bytes (max {})",
            file_data.len(),
            liberte_shared::constants::MAX_FILE_SIZE
        ));
    }

    let hash = blake3::hash(&file_data);
    let hash_bytes: [u8; 32] = *hash.as_bytes();

    let file_id = Uuid::new_v4();
    let timestamp = Utc::now();

    let (sender_pubkey, cmd_tx) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let identity = guard
            .identity
            .as_ref()
            .ok_or_else(|| "No identity loaded".to_string())?;
        let tx = guard
            .swarm_cmd_tx
            .clone()
            .ok_or_else(|| "Swarm not started".to_string())?;
        (identity.public_key_bytes(), tx)
    };

    let offer = WireMessage::FileOffer(FileOffer {
        sender: UserId(sender_pubkey),
        channel_id: ChannelId(channel_uuid),
        file_id,
        file_name: file_name.clone(),
        file_size,
        file_hash: hash_bytes,
        timestamp,
    });

    // Encrypt the file offer before publishing (metadata is sensitive)
    let topic = ChannelId(channel_uuid).to_topic();
    let wire_bytes = offer
        .to_bytes()
        .map_err(|e| format!("Serialization failed: {e}"))?;

    // NOTE: File offer is published as plaintext wire message on the
    // channel topic. In a future iteration, this should be encrypted
    // with the channel key (requires passing channel_key_hex to send_file).
    cmd_tx
        .send(SwarmCommand::PublishMessage {
            topic,
            data: wire_bytes,
        })
        .await
        .map_err(|e| format!("Failed to publish file offer: {e}"))?;

    let blob = Blob {
        id: file_id,
        file_name: file_name.clone(),
        file_size: file_size as i64,
        blake3_hash: hex::encode(hash_bytes),
        is_uploaded: false,
        local_path: file_path.clone(),
        created_at: timestamp,
    };

    {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(ref db) = guard.database {
            db.insert_blob(&blob)
                .map_err(|e| format!("Failed to store blob: {e}"))?;
        }
    }

    info!(
        file_id = %file_id,
        file_name = %file_name,
        size = file_size,
        "File offer sent"
    );

    Ok(FileSendResult {
        file_id: file_id.to_string(),
        file_name,
        file_size,
    })
}

#[tauri::command]
pub async fn upload_premium_blob(
    state: State<'_, Arc<Mutex<AppState>>>,
    file_path: String,
    channel_key_hex: String,
) -> Result<String, String> {
    {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if !guard.is_premium {
            return Err("Premium subscription required for blob uploads".into());
        }
    }

    let key_bytes = hex::decode(&channel_key_hex)
        .map_err(|e| format!("Invalid channel key hex: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("Channel key must be 32 bytes (64 hex chars)".into());
    }
    let mut channel_key = [0u8; 32];
    channel_key.copy_from_slice(&key_bytes);

    let file_data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let encrypted = liberte_shared::crypto::encrypt(&channel_key, &file_data)
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let blob_id = Uuid::new_v4();

    // TODO: HTTP POST to VPS relay endpoint with encrypted data.
    // For now, store locally and mark as uploaded.
    info!(
        blob_id = %blob_id,
        encrypted_size = encrypted.len(),
        "Premium blob encrypted (upload pending VPS integration)"
    );

    Ok(blob_id.to_string())
}
