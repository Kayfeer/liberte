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

/// Build a map of hex pubkey → display_name from the users table.
fn load_display_names(db: &liberte_store::Database) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Ok(mut stmt) = db
        .conn()
        .prepare("SELECT pubkey, display_name FROM users WHERE display_name IS NOT NULL")
    {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                map.insert(row.0, row.1);
            }
        }
    }
    // Also check app_settings for own display name (overrides users table)
    if let Ok(json_str) =
        db.conn()
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
    {
        if let Ok(settings) =
            serde_json::from_str::<crate::commands::settings::AppSettings>(&json_str)
        {
            if let Some(name) = settings.display_name {
                // Find own pubkey (from local_identity → derive pubkey)
                if let Ok(secret_hex) = db.conn().query_row(
                    "SELECT secret_key FROM local_identity WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0),
                ) {
                    if let Ok(bytes) = hex::decode(&secret_hex) {
                        if bytes.len() == 32 {
                            let mut key = [0u8; 32];
                            key.copy_from_slice(&bytes);
                            let id = liberte_shared::identity::Identity::from_secret_bytes(&key);
                            let pk = hex::encode(id.public_key_bytes());
                            map.insert(pk, name);
                        }
                    }
                }
            }
        }
    }
    map
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    pub id: String,
    pub channel_id: String,
    pub sender_id: String,
    pub sender_display_name: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub reactions: Vec<ReactionGroupDto>,
}

/// A grouped reaction (emoji + list of user pubkeys who reacted).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionGroupDto {
    pub emoji: String,
    pub users: Vec<String>,
}

impl MessageDto {
    pub fn from_message(
        m: Message,
        channel_key: Option<&[u8; 32]>,
        sender_display_name: Option<String>,
        reactions: Vec<ReactionGroupDto>,
    ) -> Self {
        let content = match channel_key {
            Some(key) => match crypto::decrypt(key, &m.encrypted_content) {
                Ok(bytes) => String::from_utf8(bytes)
                    .unwrap_or_else(|_| "[déchiffrement impossible]".to_string()),
                Err(_) => "[déchiffrement impossible]".to_string(),
            },
            None => "[clé manquante]".to_string(),
        };
        Self {
            id: m.id.to_string(),
            channel_id: m.channel_id.to_string(),
            sender_id: hex::encode(m.sender_pubkey),
            sender_display_name,
            content,
            timestamp: m.timestamp.to_rfc3339(),
            reactions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
    let channel_uuid =
        Uuid::parse_str(&channel_id).map_err(|e| format!("Invalid channel_id: {e}"))?;

    let key_bytes =
        hex::decode(&channel_key_hex).map_err(|e| format!("Invalid channel key hex: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("Channel key must be 32 bytes (64 hex chars)".into());
    }
    let mut channel_key = [0u8; 32];
    channel_key.copy_from_slice(&key_bytes);

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
    let wire_bytes = wire_msg
        .to_bytes()
        .map_err(|e| format!("Serialization failed: {e}"))?;

    cmd_tx
        .send(SwarmCommand::PublishMessage {
            topic,
            data: wire_bytes,
        })
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
    channel_key_hex: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<MessageDto>, String> {
    let channel_uuid =
        Uuid::parse_str(&channel_id).map_err(|e| format!("Invalid channel_id: {e}"))?;

    let channel_key: Option<[u8; 32]> = match channel_key_hex {
        Some(ref hex_str) if !hex_str.is_empty() => {
            let bytes =
                hex::decode(hex_str).map_err(|e| format!("Invalid channel key hex: {e}"))?;
            if bytes.len() != 32 {
                return Err("Channel key must be 32 bytes".into());
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            Some(key)
        }
        _ => None,
    };

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let messages = db
        .get_messages_for_channel(channel_uuid, limit.unwrap_or(50), offset.unwrap_or(0))
        .map_err(|e| format!("Failed to load messages: {e}"))?;

    let names = load_display_names(db);

    // Load reactions for all messages in batch
    let msg_ids: Vec<uuid::Uuid> = messages.iter().map(|m| m.id).collect();
    let reactions_map = db.get_reactions_for_messages(&msg_ids).unwrap_or_default();

    Ok(messages
        .into_iter()
        .map(|m| {
            let sender_hex = hex::encode(m.sender_pubkey);
            let name = names.get(&sender_hex).cloned();
            let reactions = group_reactions(reactions_map.get(&m.id));
            MessageDto::from_message(m, channel_key.as_ref(), name, reactions)
        })
        .collect())
}

#[tauri::command]
pub fn list_channels(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Vec<ChannelDto>, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let channels = db
        .list_channels()
        .map_err(|e| format!("Failed to list channels: {e}"))?;

    Ok(channels.into_iter().map(ChannelDto::from).collect())
}

/// Search messages across all channels (or a specific channel) by decrypting and matching.
#[tauri::command]
pub fn search_messages(
    state: State<'_, Arc<Mutex<AppState>>>,
    query: String,
    channel_id: Option<String>,
) -> Result<Vec<MessageDto>, String> {
    let query_lower = query.to_lowercase();

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    // Get channel keys to decrypt
    let channel_keys = db
        .get_all_channel_keys()
        .map_err(|e| format!("Failed to load channel keys: {e}"))?;

    // Determine which channels to search
    let target_channels: Vec<uuid::Uuid> = match channel_id {
        Some(ref cid) => {
            let uuid = Uuid::parse_str(cid).map_err(|e| format!("Invalid channel_id: {e}"))?;
            vec![uuid]
        }
        None => {
            let channels = db
                .list_channels()
                .map_err(|e| format!("Failed to list channels: {e}"))?;
            channels.into_iter().map(|c| c.id).collect()
        }
    };

    let mut results = Vec::new();

    let names = load_display_names(db);

    for ch_id in target_channels {
        let key_hex = channel_keys.get(&ch_id);
        let channel_key: Option<[u8; 32]> = key_hex.and_then(|hex_str| {
            let bytes = hex::decode(hex_str).ok()?;
            if bytes.len() != 32 {
                return None;
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            Some(key)
        });

        let messages = db
            .get_messages_for_channel(ch_id, 10_000, 0)
            .map_err(|e| format!("Failed to load messages: {e}"))?;

        for m in messages {
            let sender_hex = hex::encode(m.sender_pubkey);
            let name = names.get(&sender_hex).cloned();
            let reactions = group_reactions(db.get_reactions_for_message(m.id).ok().as_ref());
            let dto = MessageDto::from_message(m, channel_key.as_ref(), name, reactions);
            if dto.content.to_lowercase().contains(&query_lower) {
                results.push(dto);
            }
        }
    }

    // Sort by timestamp descending
    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Limit results
    results.truncate(100);

    Ok(results)
}

/// Group raw Reaction rows into ReactionGroupDto (emoji → list of user pubkeys).
fn group_reactions(reactions: Option<&Vec<liberte_store::Reaction>>) -> Vec<ReactionGroupDto> {
    let Some(reactions) = reactions else {
        return Vec::new();
    };

    let mut map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for r in reactions {
        map.entry(r.emoji.clone())
            .or_default()
            .push(r.user_pubkey.clone());
    }

    map.into_iter()
        .map(|(emoji, users)| ReactionGroupDto { emoji, users })
        .collect()
}

#[tauri::command]
pub fn add_reaction(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    message_id: String,
    emoji: String,
) -> Result<(), String> {
    let channel_uuid =
        Uuid::parse_str(&channel_id).map_err(|e| format!("Invalid channel_id: {e}"))?;
    let message_uuid =
        Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message_id: {e}"))?;

    let emoji = emoji.trim().to_string();
    if emoji.is_empty() || emoji.len() > 32 {
        return Err("Invalid emoji".into());
    }

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.add_reaction(message_uuid, channel_uuid, &pubkey_hex, &emoji)
        .map_err(|e| format!("Failed to add reaction: {e}"))?;

    info!(emoji = %emoji, message = %message_id, "Reaction added");
    Ok(())
}

#[tauri::command]
pub fn remove_reaction(
    state: State<'_, Arc<Mutex<AppState>>>,
    message_id: String,
    emoji: String,
) -> Result<(), String> {
    let message_uuid =
        Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message_id: {e}"))?;

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.remove_reaction(message_uuid, &pubkey_hex, &emoji)
        .map_err(|e| format!("Failed to remove reaction: {e}"))?;

    info!(emoji = %emoji, message = %message_id, "Reaction removed");
    Ok(())
}

#[tauri::command]
pub fn get_reactions(
    state: State<'_, Arc<Mutex<AppState>>>,
    message_id: String,
) -> Result<Vec<ReactionGroupDto>, String> {
    let message_uuid =
        Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message_id: {e}"))?;

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let reactions = db
        .get_reactions_for_message(message_uuid)
        .map_err(|e| format!("Failed to get reactions: {e}"))?;

    Ok(group_reactions(Some(&reactions)))
}
