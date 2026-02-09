//! CRUD operations for [`Server`] records.

use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::database::Database;
use crate::error::{Result, StoreError};
use crate::models::Server;

impl Database {
    // ------------------------------------------------------------------
    // Create
    // ------------------------------------------------------------------

    /// Insert a new server.
    pub fn create_server(&self, server: &Server) -> Result<()> {
        self.conn().execute(
            "INSERT INTO servers (id, name, owner_pubkey, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                server.id.to_string(),
                server.name,
                hex::encode(server.owner_pubkey),
                server.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Read
    // ------------------------------------------------------------------

    /// Fetch a single server by UUID.
    pub fn get_server(&self, id: Uuid) -> Result<Server> {
        self.conn()
            .query_row(
                "SELECT id, name, owner_pubkey, created_at
                 FROM servers
                 WHERE id = ?1",
                params![id.to_string()],
                row_to_server,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::Sqlite(other),
            })
    }

    /// List all servers ordered by name.
    pub fn list_servers(&self) -> Result<Vec<Server>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, owner_pubkey, created_at
             FROM servers
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map([], row_to_server)?;

        let mut servers = Vec::new();
        for row in rows {
            servers.push(row?);
        }
        Ok(servers)
    }

    // ------------------------------------------------------------------
    // Delete
    // ------------------------------------------------------------------

    /// Delete a server by UUID.  Returns `true` if a row was deleted.
    ///
    /// Note: due to `ON DELETE CASCADE` in the schema, all channels belonging
    /// to this server (and their messages) will also be removed.
    pub fn delete_server(&self, id: Uuid) -> Result<bool> {
        let affected = self
            .conn()
            .execute("DELETE FROM servers WHERE id = ?1", params![id.to_string()])?;
        Ok(affected > 0)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a `rusqlite::Row` to a [`Server`].
fn row_to_server(row: &rusqlite::Row<'_>) -> rusqlite::Result<Server> {
    let id_str: String = row.get(0)?;
    let name: String = row.get(1)?;
    let owner_hex: String = row.get(2)?;
    let created_str: String = row.get(3)?;

    let id = Uuid::parse_str(&id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    let owner_bytes = hex::decode(&owner_hex)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
    let mut owner_pubkey = [0u8; 32];
    if owner_bytes.len() == 32 {
        owner_pubkey.copy_from_slice(&owner_bytes);
    }

    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(Server {
        id,
        name,
        owner_pubkey,
        created_at,
    })
}
