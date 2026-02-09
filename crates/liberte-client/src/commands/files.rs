//! File transfer Tauri commands.
//!
//! Provides commands for sending files via P2P and uploading encrypted
//! blobs to the premium VPS relay for offline delivery.

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

/// Response payload returned after initiating a file transfer.
#[derive(Debug, Clone, Serialize)]
pub struct FileSendResult {
    pub file_id: String,
    pub file_name: String,
    pub file_size: u64,
}

/// Send a file to peers via P2P GossipSub.
///
/// Reads the file from `file_path`, computes a BLAKE3 hash, publishes a
/// `FileOffer` wire message on the channel topic, stores blob metadata
/// locally, and returns a summary.
///
/// # Arguments (from JS)
///
/// * `channel_id` -- UUID string of the target channel.
/// * `file_path` -- absolute path to the file on disk.
#[tauri::command]
pub async fn send_file(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    file_path: String,
) -> Result<FileSendResult, String> {
    let channel_uuid = Uuid::parse_str(&channel_id)
        .map_err(|e| format!("Invalid channel_id: {e}"))?;

    // Read the file
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

    // Check against the max file size
    if file_data.len() > liberte_shared::constants::MAX_FILE_SIZE {
        return Err(format!(
            "File too large: {} bytes (max {})",
            file_data.len(),
            liberte_shared::constants::MAX_FILE_SIZE
        ));
    }

    // Compute BLAKE3 hash
    let hash = blake3::hash(&file_data);
    let hash_bytes: [u8; 32] = *hash.as_bytes();

    let file_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // Read identity and swarm from state
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

    // Build a FileOffer wire message
    let offer = WireMessage::FileOffer(FileOffer {
        sender: UserId(sender_pubkey),
        channel_id: ChannelId(channel_uuid),
        file_id,
        file_name: file_name.clone(),
        file_size,
        file_hash: hash_bytes,
        timestamp,
    });

    let topic = ChannelId(channel_uuid).to_topic();
    let wire_bytes = offer
        .to_bytes()
        .map_err(|e| format!("Serialization failed: {e}"))?;

    cmd_tx
        .send(SwarmCommand::PublishMessage {
            topic,
            data: wire_bytes,
        })
        .await
        .map_err(|e| format!("Failed to publish file offer: {e}"))?;

    // Persist blob metadata locally
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

/// Upload an encrypted file blob to the premium VPS for offline delivery.
///
/// This is a premium-only feature.  The file is encrypted locally before
/// upload and the VPS never sees plaintext.
///
/// # Arguments (from JS)
///
/// * `file_path` -- absolute path to the file on disk.
/// * `channel_key_hex` -- 32-byte channel key for encryption (hex).
#[tauri::command]
pub async fn upload_premium_blob(
    state: State<'_, Arc<Mutex<AppState>>>,
    file_path: String,
    channel_key_hex: String,
) -> Result<String, String> {
    // Verify premium status
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

    // Read and encrypt the file
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
