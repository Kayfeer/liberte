//! CRUD operations for [`Channel`] records.

use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::database::Database;
use crate::error::{Result, StoreError};
use crate::models::Channel;

impl Database {
    // ------------------------------------------------------------------
    // Create
    // ------------------------------------------------------------------

    /// Insert a new channel.
    pub fn create_channel(&self, channel: &Channel) -> Result<()> {
        self.conn().execute(
            "INSERT INTO channels (id, name, server_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                channel.id.to_string(),
                channel.name,
                channel.server_id.map(|s| s.to_string()),
                channel.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Read
    // ------------------------------------------------------------------

    /// Fetch a single channel by UUID.
    pub fn get_channel(&self, id: Uuid) -> Result<Channel> {
        self.conn()
            .query_row(
                "SELECT id, name, server_id, created_at
                 FROM channels
                 WHERE id = ?1",
                params![id.to_string()],
                row_to_channel,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::Sqlite(other),
            })
    }

    /// List all channels, ordered by creation date descending.
    pub fn list_channels(&self) -> Result<Vec<Channel>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, server_id, created_at
             FROM channels
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], row_to_channel)?;

        let mut channels = Vec::new();
        for row in rows {
            channels.push(row?);
        }
        Ok(channels)
    }

    /// List channels belonging to a specific server, ordered by name.
    pub fn list_channels_for_server(&self, server_id: Uuid) -> Result<Vec<Channel>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, server_id, created_at
             FROM channels
             WHERE server_id = ?1
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map(params![server_id.to_string()], row_to_channel)?;

        let mut channels = Vec::new();
        for row in rows {
            channels.push(row?);
        }
        Ok(channels)
    }

    // ------------------------------------------------------------------
    // Delete
    // ------------------------------------------------------------------

    /// Delete a channel by UUID.  Returns `true` if a row was deleted.
    pub fn delete_channel(&self, id: Uuid) -> Result<bool> {
        let affected = self
            .conn()
            .execute("DELETE FROM channels WHERE id = ?1", params![id.to_string()])?;
        Ok(affected > 0)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a `rusqlite::Row` to a [`Channel`].
fn row_to_channel(row: &rusqlite::Row<'_>) -> rusqlite::Result<Channel> {
    let id_str: String = row.get(0)?;
    let name: String = row.get(1)?;
    let server_id_str: Option<String> = row.get(2)?;
    let created_str: String = row.get(3)?;

    let id = Uuid::parse_str(&id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    let server_id = server_id_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;

    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(Channel {
        id,
        name,
        server_id,
        created_at,
    })
}
