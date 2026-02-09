use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use rusqlite::Connection;

use crate::error::{Result, StoreError};
use crate::migrations;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_key: &[u8; 32]) -> Result<Self> {
        let project_dirs =
            ProjectDirs::from("com", "liberte", "liberte").ok_or(StoreError::NoDataDir)?;

        let data_dir = project_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        let db_path = data_dir.join("liberte.db");

        tracing::info!(path = %db_path.display(), "opening database");

        Self::open_at(&db_path, db_key)
    }

    pub fn open_at(path: &Path, db_key: &[u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Activate SQLCipher encryption when the feature is enabled
        #[cfg(feature = "sqlcipher")]
        {
            let key_hex = hex::encode(db_key);
            conn.pragma_update(None, "key", &format!("x'{key_hex}'"))?;
        }
        #[cfg(not(feature = "sqlcipher"))]
        {
            let _ = db_key; // suppress unused warning
        }

        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        migrations::run_migrations(&conn)?;

        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.conn.path().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let key = [0xABu8; 32];

        let db = Database::open_at(&path, &key).expect("should open");
        assert!(db.path().is_some());
    }
}
