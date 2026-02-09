use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::State;
use tracing::info;
use uuid::Uuid;

use liberte_net::SwarmCommand;
use liberte_shared::crypto;
use liberte_shared::protocol::{ChatMessage, WireMessage};
use liberte_shared::types::ChannelId;
use liberte_store::{Channel, Message};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct MessageDto {
    pub id: String,
    pub channel_id: String,
    pub sender: String,
    pub encrypted_content: Vec<u8>,
    pub timestamp: String,
}

impl From<Message> for MessageDto {
    fn from(m: Message) -> Self {
        Self {
            id: m.id.to_string(),
            channel_id: m.channel_id.to_string(),
            sender: hex::encode(m.sender_pubkey),
            encrypted_content: m.encrypted_content,
            timestamp: m.timestamp.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelDto {
    pub id: String,
    pub name: String,
    pub server_id: Option<String>,
    pub created_at: String,
}

impl From<Channel> for ChannelDto {
    fn from(c: Channel) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            server_id: c.server_id.map(|s| s.to_string()),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    content: String,
    channel_key_hex: String,
) -> Result<String, String> {
    let channel_uuid = Uuid::parse_str(&channel_id)
        .map_err(|e| format!("Invalid channel_id: {e}"))?;

    let key_bytes = hex::decode(&channel_key_hex)
        .map_err(|e| format!("Invalid channel key hex: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("Channel key must be 32 bytes (64 hex chars)".into());
    }
    let mut channel_key = [0u8; 32];
    channel_key.copy_from_slice(&key_bytes);

    let (sender_pubkey, cmd_tx) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let identity = guard.identity.as_ref()
            .ok_or_else(|| "No identity loaded".to_string())?;
        let tx = guard.swarm_cmd_tx.clone()
            .ok_or_else(|| "Swarm not started".to_string())?;
        (identity.public_key_bytes(), tx)
    };

    let encrypted = crypto::encrypt(&channel_key, content.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let message_id = Uuid::new_v4();
    let timestamp = Utc::now();

    let wire_msg = WireMessage::ChatMessage(ChatMessage {
        sender: liberte_shared::types::UserId(sender_pubkey),
        channel_id: ChannelId(channel_uuid),
        encrypted_content: encrypted.clone(),
        timestamp,
        message_id,
    });

    let topic = ChannelId(channel_uuid).to_topic();
    let wire_bytes = wire_msg.to_bytes()
        .map_err(|e| format!("Serialization failed: {e}"))?;

    cmd_tx
        .send(SwarmCommand::PublishMessage { topic, data: wire_bytes })
        .await
        .map_err(|e| format!("Failed to publish message: {e}"))?;

    let msg = Message {
        id: message_id,
        channel_id: channel_uuid,
        sender_pubkey,
        encrypted_content: encrypted,
        timestamp,
    };

    {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(ref db) = guard.database {
            db.insert_message(&msg)
                .map_err(|e| format!("Failed to store message: {e}"))?;
        }
    }

    info!(msg_id = %message_id, channel = %channel_id, "Message sent");
    Ok(message_id.to_string())
}

#[tauri::command]
pub fn get_messages(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<MessageDto>, String> {
    let channel_uuid = Uuid::parse_str(&channel_id)
        .map_err(|e| format!("Invalid channel_id: {e}"))?;

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard.database.as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let messages = db
        .get_messages_for_channel(channel_uuid, limit.unwrap_or(50), offset.unwrap_or(0))
        .map_err(|e| format!("Failed to load messages: {e}"))?;

    Ok(messages.into_iter().map(MessageDto::from).collect())
}

#[tauri::command]
pub fn list_channels(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<ChannelDto>, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard.database.as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let channels = db.list_channels()
        .map_err(|e| format!("Failed to list channels: {e}"))?;

    Ok(channels.into_iter().map(ChannelDto::from).collect())
}
