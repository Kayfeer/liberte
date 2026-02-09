use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::database::Database;
use crate::error::{Result, StoreError};
use crate::models::Message;

impl Database {
    pub fn insert_message(&self, message: &Message) -> Result<()> {
        self.conn().execute(
            "INSERT INTO messages (id, channel_id, sender_pubkey, encrypted_content, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id.to_string(),
                message.channel_id.to_string(),
                hex::encode(message.sender_pubkey),
                message.encrypted_content,
                message.timestamp.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_messages_for_channel(
        &self,
        channel_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Message>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, channel_id, sender_pubkey, encrypted_content, timestamp
             FROM messages
             WHERE channel_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let rows = stmt.query_map(
            params![channel_id.to_string(), limit, offset],
            row_to_message,
        )?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn get_message_by_id(&self, id: Uuid) -> Result<Message> {
        self.conn()
            .query_row(
                "SELECT id, channel_id, sender_pubkey, encrypted_content, timestamp
                 FROM messages WHERE id = ?1",
                params![id.to_string()],
                row_to_message,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::Sqlite(other),
            })
    }

    pub fn delete_message(&self, id: Uuid) -> Result<bool> {
        let affected = self.conn().execute(
            "DELETE FROM messages WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(affected > 0)
    }
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let id_str: String = row.get(0)?;
    let channel_id_str: String = row.get(1)?;
    let sender_hex: String = row.get(2)?;
    let encrypted_content: Vec<u8> = row.get(3)?;
    let ts_str: String = row.get(4)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let channel_id = Uuid::parse_str(&channel_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let sender_bytes = hex::decode(&sender_hex).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let mut sender_pubkey = [0u8; 32];
    if sender_bytes.len() == 32 {
        sender_pubkey.copy_from_slice(&sender_bytes);
    }

    let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339(&ts_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?;

    Ok(Message {
        id,
        channel_id,
        sender_pubkey,
        encrypted_content,
        timestamp,
    })
}
