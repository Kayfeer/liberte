use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::database::Database;
use crate::error::{Result, StoreError};
use crate::models::Reaction;

impl Database {
    pub fn add_reaction(
        &self,
        message_id: Uuid,
        channel_id: Uuid,
        user_pubkey: &str,
        emoji: &str,
    ) -> Result<Reaction> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        self.conn().execute(
            "INSERT OR IGNORE INTO reactions (id, message_id, channel_id, user_pubkey, emoji, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                message_id.to_string(),
                channel_id.to_string(),
                user_pubkey,
                emoji,
                now.to_rfc3339(),
            ],
        )?;

        Ok(Reaction {
            id,
            message_id,
            channel_id,
            user_pubkey: user_pubkey.to_string(),
            emoji: emoji.to_string(),
            created_at: now,
        })
    }

    pub fn remove_reaction(
        &self,
        message_id: Uuid,
        user_pubkey: &str,
        emoji: &str,
    ) -> Result<bool> {
        let affected = self.conn().execute(
            "DELETE FROM reactions WHERE message_id = ?1 AND user_pubkey = ?2 AND emoji = ?3",
            params![message_id.to_string(), user_pubkey, emoji],
        )?;
        Ok(affected > 0)
    }

    pub fn get_reactions_for_message(&self, message_id: Uuid) -> Result<Vec<Reaction>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, message_id, channel_id, user_pubkey, emoji, created_at
             FROM reactions WHERE message_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map(params![message_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let msg_id_str: String = row.get(1)?;
            let ch_id_str: String = row.get(2)?;
            let user_pk: String = row.get(3)?;
            let emoji: String = row.get(4)?;
            let ts_str: String = row.get(5)?;

            let id = Uuid::parse_str(&id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let message_id = Uuid::parse_str(&msg_id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let channel_id = Uuid::parse_str(&ch_id_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

            Ok(Reaction {
                id,
                message_id,
                channel_id,
                user_pubkey: user_pk,
                emoji,
                created_at,
            })
        })?;

        let mut reactions = Vec::new();
        for row in rows {
            reactions.push(row?);
        }
        Ok(reactions)
    }

    /// Get reactions for multiple messages at once (batch query).
    pub fn get_reactions_for_messages(
        &self,
        message_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<Reaction>>> {
        let mut map = std::collections::HashMap::new();
        for id in message_ids {
            let reactions = self.get_reactions_for_message(*id)?;
            if !reactions.is_empty() {
                map.insert(*id, reactions);
            }
        }
        Ok(map)
    }

    /// Update user bio.
    pub fn set_user_bio(&self, pubkey_hex: &str, bio: Option<&str>) -> Result<()> {
        self.conn().execute(
            "UPDATE users SET bio = ?1 WHERE pubkey = ?2",
            params![bio, pubkey_hex],
        )?;
        Ok(())
    }

    /// Update user status (online, dnd, idle, invisible).
    pub fn set_user_status(&self, pubkey_hex: &str, status: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE users SET status = ?1 WHERE pubkey = ?2",
            params![status, pubkey_hex],
        )?;
        Ok(())
    }

    /// Get user bio and status.
    pub fn get_user_profile(&self, pubkey_hex: &str) -> Result<(Option<String>, String)> {
        self.conn()
            .query_row(
                "SELECT bio, status FROM users WHERE pubkey = ?1",
                params![pubkey_hex],
                |row| {
                    let bio: Option<String> = row.get(0)?;
                    let status: String = row.get(1).unwrap_or_else(|_| "online".to_string());
                    Ok((bio, status))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::Sqlite(other),
            })
    }
}
