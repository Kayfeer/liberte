use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::Database;
use crate::error::Result;

/// Full backup payload — serialized to JSON then encrypted client-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPayload {
    /// ISO 8601 timestamp of when the backup was created
    pub created_at: String,
    /// App version that produced the backup
    pub version: String,
    pub channels: Vec<BackupChannel>,
    pub messages: Vec<BackupMessage>,
    pub channel_keys: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupChannel {
    pub id: String,
    pub name: String,
    pub server_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMessage {
    pub id: String,
    pub channel_id: String,
    pub sender_pubkey_hex: String,
    pub encrypted_content_hex: String,
    pub timestamp: String,
}

impl Database {
    /// Export all channels, messages and channel keys into a serializable struct.
    pub fn export_backup(&self) -> Result<BackupPayload> {
        let channels = self.list_channels()?;
        let channel_keys = self.get_all_channel_keys()?;

        let mut backup_messages = Vec::new();
        for ch in &channels {
            // Get all messages (large limit)
            let msgs = self.get_messages_for_channel(ch.id, 1_000_000, 0)?;
            for m in msgs {
                backup_messages.push(BackupMessage {
                    id: m.id.to_string(),
                    channel_id: m.channel_id.to_string(),
                    sender_pubkey_hex: hex::encode(m.sender_pubkey),
                    encrypted_content_hex: hex::encode(&m.encrypted_content),
                    timestamp: m.timestamp.to_rfc3339(),
                });
            }
        }

        let backup_channels = channels
            .iter()
            .map(|c| BackupChannel {
                id: c.id.to_string(),
                name: c.name.clone(),
                server_id: c.server_id.map(|s| s.to_string()),
                created_at: c.created_at.to_rfc3339(),
            })
            .collect();

        let key_map = channel_keys
            .into_iter()
            .map(|(id, key)| (id.to_string(), key))
            .collect();

        Ok(BackupPayload {
            created_at: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            channels: backup_channels,
            messages: backup_messages,
            channel_keys: key_map,
        })
    }

    /// Import a backup payload, merging with existing data (INSERT OR IGNORE).
    pub fn import_backup(&self, payload: &BackupPayload) -> Result<ImportStats> {
        let mut stats = ImportStats::default();

        for ch in &payload.channels {
            let id = Uuid::parse_str(&ch.id)?;
            let server_id = ch.server_id.as_deref().map(Uuid::parse_str).transpose()?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&ch.created_at)?;

            let channel = crate::models::Channel {
                id,
                name: ch.name.clone(),
                server_id,
                created_at: created_at.with_timezone(&chrono::Utc),
            };

            // INSERT OR IGNORE — don't overwrite existing
            let res = self.conn().execute(
                "INSERT OR IGNORE INTO channels (id, name, server_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    channel.id.to_string(),
                    channel.name,
                    channel.server_id.map(|s| s.to_string()),
                    channel.created_at.to_rfc3339(),
                ],
            );
            if matches!(res, Ok(1)) {
                stats.channels_imported += 1;
            }
        }

        for (id_str, key) in &payload.channel_keys {
            if let Ok(id) = Uuid::parse_str(id_str) {
                let _ = self.store_channel_key(id, key);
                stats.keys_imported += 1;
            }
        }

        for msg in &payload.messages {
            let id = Uuid::parse_str(&msg.id)?;
            let channel_id = Uuid::parse_str(&msg.channel_id)?;
            let sender_pubkey = hex::decode(&msg.sender_pubkey_hex)?;
            let encrypted_content = hex::decode(&msg.encrypted_content_hex)?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&msg.timestamp)?;

            if sender_pubkey.len() != 32 {
                continue;
            }
            let mut pubkey = [0u8; 32];
            pubkey.copy_from_slice(&sender_pubkey);

            let message = crate::models::Message {
                id,
                channel_id,
                sender_pubkey: pubkey,
                encrypted_content,
                timestamp: timestamp.with_timezone(&chrono::Utc),
            };

            let res = self.conn().execute(
                "INSERT OR IGNORE INTO messages (id, channel_id, sender_pubkey, encrypted_content, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    message.id.to_string(),
                    message.channel_id.to_string(),
                    hex::encode(message.sender_pubkey),
                    message.encrypted_content,
                    message.timestamp.to_rfc3339(),
                ],
            );
            if matches!(res, Ok(1)) {
                stats.messages_imported += 1;
            }
        }

        Ok(stats)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImportStats {
    pub channels_imported: usize,
    pub messages_imported: usize,
    pub keys_imported: usize,
}
