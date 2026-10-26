use crate::{
    database::Database,
    error::{Result, StoreError},
    models::{Channel, Message, Reaction, Server, User},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Sensitive payload: callers must encrypt before writing or returning it.
/// Attachment files are not included; local file paths cannot restore their contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPayload {
    pub created_at: String,
    pub version: String,
    pub channels: Vec<BackupChannel>,
    pub messages: Vec<BackupMessage>,
    pub channel_keys: HashMap<String, String>,
    #[serde(default)]
    pub servers: Vec<Server>,
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
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
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportStats {
    pub channels_imported: usize,
    pub messages_imported: usize,
    pub keys_imported: usize,
    pub servers_imported: usize,
    pub users_imported: usize,
    pub reactions_imported: usize,
}
fn invalid(message: impl Into<String>) -> StoreError {
    StoreError::InvalidBackup(message.into())
}
pub(crate) fn decode_key(value: &str) -> Result<[u8; 32]> {
    hex::decode(value)?
        .try_into()
        .map_err(|_| invalid("Expected a 32-byte key"))
}
fn unique(seen: &mut HashSet<Uuid>, id: Uuid, kind: &str) -> Result<()> {
    if !seen.insert(id) {
        return Err(invalid(format!("Duplicate {kind} id: {id}")));
    }
    Ok(())
}
struct ValidatedBackup {
    channels: Vec<Channel>,
    messages: Vec<Message>,
    keys: HashMap<Uuid, String>,
}
impl BackupPayload {
    fn validate(&self) -> Result<ValidatedBackup> {
        DateTime::parse_from_rfc3339(&self.created_at)?;
        let mut seen = HashSet::new();
        let mut channels = Vec::with_capacity(self.channels.len());
        for ch in &self.channels {
            let id = Uuid::parse_str(&ch.id)?;
            unique(&mut seen, id, "channel")?;
            if ch.name.trim().is_empty() || ch.name.len() > 512 {
                return Err(invalid("Invalid channel name"));
            }
            channels.push(Channel {
                id,
                name: ch.name.clone(),
                server_id: ch.server_id.as_deref().map(Uuid::parse_str).transpose()?,
                created_at: DateTime::parse_from_rfc3339(&ch.created_at)?.with_timezone(&Utc),
            });
        }
        let mut keys = HashMap::new();
        for (id, key) in &self.channel_keys {
            if keys
                .insert(Uuid::parse_str(id)?, hex::encode(decode_key(key)?))
                .is_some()
            {
                return Err(invalid("Duplicate channel key"));
            }
        }
        seen.clear();
        let mut messages = Vec::with_capacity(self.messages.len());
        for msg in &self.messages {
            let id = Uuid::parse_str(&msg.id)?;
            unique(&mut seen, id, "message")?;
            let encrypted_content = hex::decode(&msg.encrypted_content_hex)?;
            if encrypted_content.len() < 40 {
                return Err(invalid("Truncated encrypted message"));
            }
            messages.push(Message {
                id,
                channel_id: Uuid::parse_str(&msg.channel_id)?,
                sender_pubkey: decode_key(&msg.sender_pubkey_hex)?,
                encrypted_content,
                timestamp: DateTime::parse_from_rfc3339(&msg.timestamp)?.with_timezone(&Utc),
            });
        }
        seen.clear();
        for server in &self.servers {
            unique(&mut seen, server.id, "server")?;
        }
        let mut users = HashSet::new();
        for user in &self.users {
            if !users.insert(user.pubkey) {
                return Err(invalid("Duplicate user"));
            }
            if !["online", "dnd", "idle", "invisible", "offline"].contains(&user.status.as_str()) {
                return Err(invalid("Invalid user status"));
            }
        }
        seen.clear();
        let mut reactions = HashSet::new();
        for reaction in &self.reactions {
            unique(&mut seen, reaction.id, "reaction")?;
            decode_key(&reaction.user_pubkey)?;
            if reaction.emoji.is_empty() || reaction.emoji.len() > 128 {
                return Err(invalid("Invalid reaction"));
            }
            if !reactions.insert((
                reaction.message_id,
                reaction.user_pubkey.to_lowercase(),
                &reaction.emoji,
            )) {
                return Err(invalid("Duplicate reaction"));
            }
        }
        Ok(ValidatedBackup {
            channels,
            messages,
            keys,
        })
    }
}
impl Database {
    pub fn export_backup(&self) -> Result<BackupPayload> {
        let tx = self.conn().unchecked_transaction()?;
        let channels = self
            .list_channels()?
            .into_iter()
            .map(|c| BackupChannel {
                id: c.id.to_string(),
                name: c.name,
                server_id: c.server_id.map(|id| id.to_string()),
                created_at: c.created_at.to_rfc3339(),
            })
            .collect();
        let messages = {
            let mut stmt = tx.prepare("SELECT id, channel_id, sender_pubkey, encrypted_content, timestamp FROM messages ORDER BY timestamp, id")?;
            let rows = stmt.query_map([], crate::messages::row_to_message)?;
            let mut messages = Vec::new();
            for row in rows {
                let m = row?;
                messages.push(BackupMessage {
                    id: m.id.to_string(),
                    channel_id: m.channel_id.to_string(),
                    sender_pubkey_hex: hex::encode(m.sender_pubkey),
                    encrypted_content_hex: hex::encode(m.encrypted_content),
                    timestamp: m.timestamp.to_rfc3339(),
                });
            }
            messages
        };
        let users = {
            let mut stmt = tx.prepare(
                "SELECT pubkey, display_name, avatar_hash, bio, status, created_at FROM users",
            )?;
            let mut rows = stmt.query([])?;
            let mut users = Vec::new();
            while let Some(row) = rows.next()? {
                users.push(User {
                    pubkey: decode_key(&row.get::<_, String>(0)?)?,
                    display_name: row.get(1)?,
                    avatar_hash: row.get(2)?,
                    bio: row.get(3)?,
                    status: row.get(4)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)?
                        .with_timezone(&Utc),
                });
            }
            users
        };
        let reactions = {
            let mut stmt = tx.prepare(
                "SELECT id, message_id, channel_id, user_pubkey, emoji, created_at FROM reactions",
            )?;
            let mut rows = stmt.query([])?;
            let mut reactions = Vec::new();
            while let Some(row) = rows.next()? {
                reactions.push(Reaction {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?)?,
                    message_id: Uuid::parse_str(&row.get::<_, String>(1)?)?,
                    channel_id: Uuid::parse_str(&row.get::<_, String>(2)?)?,
                    user_pubkey: row.get(3)?,
                    emoji: row.get(4)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)?
                        .with_timezone(&Utc),
                });
            }
            reactions
        };
        let payload = BackupPayload {
            created_at: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            channels,
            messages,
            channel_keys: self
                .get_all_channel_keys()?
                .into_iter()
                .map(|(id, key)| (id.to_string(), key))
                .collect(),
            servers: self.list_servers()?,
            users,
            reactions,
        };
        payload.validate()?;
        tx.commit()?;
        Ok(payload)
    }
    pub fn import_backup(&self, payload: &BackupPayload) -> Result<ImportStats> {
        self.import_backup_with(payload, |_| Ok(()))
    }

    /// Identity/settings restoration can join this transaction. Every error rolls back.
    pub fn import_backup_with<F>(&self, payload: &BackupPayload, extra: F) -> Result<ImportStats>
    where
        F: FnOnce(&Connection) -> Result<()>,
    {
        let validated = payload.validate()?;
        let tx = self.conn().unchecked_transaction()?;
        let mut stats = ImportStats::default();
        let channels: HashSet<_> = validated.channels.iter().map(|ch| ch.id).collect();
        let servers: HashSet<_> = payload.servers.iter().map(|s| s.id).collect();
        // Resolve references and immutable conflicts before any writes.
        for ch in &validated.channels {
            if let Some(server) = ch.server_id {
                if !servers.contains(&server) {
                    self.get_server(server)?;
                }
            }
            match self.get_channel(ch.id) {
                Ok(old) if old.server_id != ch.server_id => {
                    return Err(invalid("Channel belongs to a different server"))
                }
                Err(StoreError::NotFound) | Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        for id in validated
            .keys
            .keys()
            .chain(validated.messages.iter().map(|m| &m.channel_id))
        {
            if !channels.contains(id) {
                self.get_channel(*id)?;
            }
        }
        for (id, key) in &validated.keys {
            match self.get_channel_key(*id) {
                Ok(old) if decode_key(&old)? != decode_key(key)? => {
                    return Err(invalid("Existing channel has a different encryption key"))
                }
                Err(StoreError::NotFound) | Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        // A structurally valid encrypted record can still be corrupt or use the
        // wrong channel key. Detect this before committing an unreadable history.
        let mut message_keys = validated.keys.clone();
        for msg in &validated.messages {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                message_keys.entry(msg.channel_id)
            {
                match self.get_channel_key(msg.channel_id) {
                    Ok(key) => {
                        entry.insert(key);
                    }
                    Err(StoreError::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            if let Some(key) = message_keys.get(&msg.channel_id) {
                liberte_shared::crypto::decrypt(&decode_key(key)?, &msg.encrypted_content)
                    .map_err(|_| invalid("Message authentication failed for its channel key"))?;
            }
            match self.get_message_by_id(msg.id) {
                Ok(old) if old != *msg => {
                    return Err(invalid("Existing message has different contents"))
                }
                Err(StoreError::NotFound) | Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        let message_channels: HashMap<_, _> = validated
            .messages
            .iter()
            .map(|m| (m.id, m.channel_id))
            .collect();
        for reaction in &payload.reactions {
            let channel = match message_channels.get(&reaction.message_id) {
                Some(channel) => *channel,
                None => self.get_message_by_id(reaction.message_id)?.channel_id,
            };
            if channel != reaction.channel_id {
                return Err(invalid("Reaction channel does not match its message"));
            }
            let existing: Option<(String, String, String, String)> = tx
                .query_row(
                    "SELECT message_id,channel_id,user_pubkey,emoji FROM reactions WHERE id=?1",
                    params![reaction.id.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            if let Some(old) = existing {
                if old
                    != (
                        reaction.message_id.to_string(),
                        reaction.channel_id.to_string(),
                        reaction.user_pubkey.to_lowercase(),
                        reaction.emoji.clone(),
                    )
                {
                    return Err(invalid("Reaction id conflict"));
                }
            }
        }
        for server in &payload.servers {
            match self.get_server(server.id) {
                Ok(old) if old.owner_pubkey != server.owner_pubkey => {
                    return Err(invalid("Server owner conflict"))
                }
                Err(StoreError::NotFound) | Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        for server in &payload.servers {
            stats.servers_imported += tx.execute("INSERT INTO servers (id,name,owner_pubkey,created_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO NOTHING",
                params![server.id.to_string(), server.name, hex::encode(server.owner_pubkey), server.created_at.to_rfc3339()])?;
        }
        for user in &payload.users {
            stats.users_imported += tx.execute("INSERT INTO users (pubkey,display_name,avatar_hash,bio,status,created_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(pubkey) DO NOTHING",
                params![hex::encode(user.pubkey), user.display_name, user.avatar_hash, user.bio, user.status, user.created_at.to_rfc3339()])?;
        }
        for ch in &validated.channels {
            stats.channels_imported += tx.execute("INSERT INTO channels (id,name,server_id,created_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO NOTHING",
                params![ch.id.to_string(), ch.name, ch.server_id.map(|id| id.to_string()), ch.created_at.to_rfc3339()])?;
        }
        for (id, key) in &validated.keys {
            stats.keys_imported += tx.execute("INSERT INTO channel_keys (channel_id,key_hex) VALUES (?1,?2) ON CONFLICT(channel_id) DO NOTHING", params![id.to_string(), key])?;
        }
        for msg in &validated.messages {
            stats.messages_imported += tx.execute("INSERT INTO messages (id,channel_id,sender_pubkey,encrypted_content,timestamp) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO NOTHING",
                params![msg.id.to_string(), msg.channel_id.to_string(), hex::encode(msg.sender_pubkey), msg.encrypted_content, msg.timestamp.to_rfc3339()])?;
        }
        for reaction in &payload.reactions {
            stats.reactions_imported += tx.execute("INSERT INTO reactions (id,message_id,channel_id,user_pubkey,emoji,created_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT DO NOTHING",
                params![reaction.id.to_string(), reaction.message_id.to_string(), reaction.channel_id.to_string(), reaction.user_pubkey.to_lowercase(), reaction.emoji, reaction.created_at.to_rfc3339()])?;
        }
        extra(&tx)?;
        tx.commit()?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_at(&dir.path().join("test.db"), &[0; 32]).unwrap();
        (dir, db)
    }
    fn seed(db: &Database) -> BackupPayload {
        let channel = Channel {
            id: Uuid::new_v4(),
            name: "portable".into(),
            server_id: None,
            created_at: Utc::now(),
        };
        db.create_channel(&channel).unwrap();
        db.store_channel_key(channel.id, &hex::encode([7; 32]))
            .unwrap();
        let message = Message {
            id: Uuid::new_v4(),
            channel_id: channel.id,
            sender_pubkey: [8; 32],
            encrypted_content: liberte_shared::crypto::encrypt(&[7; 32], b"portable history")
                .unwrap(),
            timestamp: Utc::now(),
        };
        db.insert_message(&message).unwrap();
        db.export_backup().unwrap()
    }
    #[test]
    fn portable_restore_and_duplicate_counts() {
        let (_one, source) = db();
        let payload = seed(&source);
        let (_two, target) = db();
        let stats = target.import_backup(&payload).unwrap();
        assert_eq!(
            (
                stats.channels_imported,
                stats.messages_imported,
                stats.keys_imported
            ),
            (1, 1, 1)
        );
        assert_eq!(
            target.import_backup(&payload).unwrap(),
            ImportStats::default()
        );
        let restored = target
            .get_message_by_id(Uuid::parse_str(&payload.messages[0].id).unwrap())
            .unwrap();
        let key = decode_key(&target.get_channel_key(restored.channel_id).unwrap()).unwrap();
        assert_eq!(
            liberte_shared::crypto::decrypt(&key, &restored.encrypted_content).unwrap(),
            b"portable history"
        );
    }
    #[test]
    fn invalid_record_and_transaction_failure_leave_no_partial_import() {
        let (_one, source) = db();
        let mut payload = seed(&source);
        let (_two, target) = db();
        payload.messages[0].sender_pubkey_hex = "00".into();
        assert!(target.import_backup(&payload).is_err());
        assert!(target.list_channels().unwrap().is_empty());
        payload = source.export_backup().unwrap();
        let result = target.import_backup_with(&payload, |conn| {
            conn.execute("INSERT INTO nonexistent VALUES (1)", [])?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(target.list_channels().unwrap().is_empty());
    }
    #[test]
    fn corrupted_ciphertext_or_wrong_archived_key_never_commits() {
        let (_one, source) = db();
        let payload = seed(&source);
        let (_two, target) = db();
        let mut corrupted = payload.clone();
        let mut bytes = hex::decode(&corrupted.messages[0].encrypted_content_hex).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        corrupted.messages[0].encrypted_content_hex = hex::encode(bytes);
        assert!(target.import_backup(&corrupted).is_err());
        assert!(target.list_channels().unwrap().is_empty());
        assert!(target.get_all_channel_keys().unwrap().is_empty());
        let mut wrong_key = payload.clone();
        *wrong_key.channel_keys.values_mut().next().unwrap() = hex::encode([99; 32]);
        assert!(target.import_backup(&wrong_key).is_err());
        assert!(target.list_channels().unwrap().is_empty());
        target.import_backup(&payload).unwrap();
        // A message-only import must authenticate against an existing local key too.
        corrupted.channel_keys.clear();
        corrupted.messages[0].id = Uuid::new_v4().to_string();
        assert!(target.import_backup(&corrupted).is_err());
        assert_eq!(target.export_backup().unwrap().messages.len(), 1);
    }

    #[test]
    fn key_conflict_preserves_existing_history() {
        let (_one, target) = db();
        let mut payload = seed(&target);
        *payload.channel_keys.values_mut().next().unwrap() = hex::encode([9; 32]);
        assert!(target.import_backup(&payload).is_err());
        assert_eq!(
            target
                .get_all_channel_keys()
                .unwrap()
                .values()
                .next()
                .unwrap(),
            &hex::encode([7; 32])
        );
        assert_eq!(target.export_backup().unwrap().messages.len(), 1);
    }
    #[test]
    fn missing_server_rolls_back_and_legacy_shape_still_parses() {
        let (_one, source) = db();
        let mut payload = seed(&source);
        let (_two, target) = db();
        payload.channels[0].server_id = Some(Uuid::new_v4().to_string());
        assert!(target.import_backup(&payload).is_err());
        assert!(target.list_channels().unwrap().is_empty());
        let mut value = serde_json::to_value(&payload).unwrap();
        for field in ["servers", "users", "reactions"] {
            value.as_object_mut().unwrap().remove(field);
        }
        assert!(serde_json::from_value::<BackupPayload>(value).is_ok());
    }
}
