use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::database::Database;
use crate::error::{Result, StoreError};
use crate::models::Blob;

impl Database {
    pub fn insert_blob(&self, blob: &Blob) -> Result<()> {
        self.conn().execute(
            "INSERT INTO blobs (id, file_name, file_size, blake3_hash, is_uploaded, local_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                blob.id.to_string(),
                blob.file_name,
                blob.file_size,
                blob.blake3_hash,
                blob.is_uploaded as i32,
                blob.local_path,
                blob.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_blob(&self, id: Uuid) -> Result<Blob> {
        self.conn()
            .query_row(
                "SELECT id, file_name, file_size, blake3_hash, is_uploaded, local_path, created_at
                 FROM blobs
                 WHERE id = ?1",
                params![id.to_string()],
                row_to_blob,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::Sqlite(other),
            })
    }

    pub fn list_blobs(&self) -> Result<Vec<Blob>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, file_name, file_size, blake3_hash, is_uploaded, local_path, created_at
             FROM blobs
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], row_to_blob)?;

        let mut blobs = Vec::new();
        for row in rows {
            blobs.push(row?);
        }
        Ok(blobs)
    }

    pub fn mark_uploaded(&self, id: Uuid) -> Result<bool> {
        let affected = self.conn().execute(
            "UPDATE blobs SET is_uploaded = 1 WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(affected > 0)
    }

    // only removes the db record, not the file on disk
    pub fn delete_blob(&self, id: Uuid) -> Result<bool> {
        let affected = self
            .conn()
            .execute("DELETE FROM blobs WHERE id = ?1", params![id.to_string()])?;
        Ok(affected > 0)
    }
}

fn row_to_blob(row: &rusqlite::Row<'_>) -> rusqlite::Result<Blob> {
    let id_str: String = row.get(0)?;
    let file_name: String = row.get(1)?;
    let file_size: i64 = row.get(2)?;
    let blake3_hash: String = row.get(3)?;
    let is_uploaded_int: i32 = row.get(4)?;
    let local_path: String = row.get(5)?;
    let created_str: String = row.get(6)?;

    let id = Uuid::parse_str(&id_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(Blob {
        id,
        file_name,
        file_size,
        blake3_hash,
        is_uploaded: is_uploaded_int != 0,
        local_path,
        created_at,
    })
}
