//! Database connection management.
//!
//! The [`Database`] struct owns a [`rusqlite::Connection`] and guarantees that
//! migrations are run before any other operation.
//!
//! Note: SQLCipher (encrypted SQLite) requires OpenSSL at build time. For
//! environments where OpenSSL is unavailable, we fall back to plain SQLite
//! with application-layer encryption of sensitive fields via XChaCha20-Poly1305.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use rusqlite::Connection;

use crate::error::{Result, StoreError};
use crate::migrations;

/// Wrapper around a [`rusqlite::Connection`].
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) the default application database.
    ///
    /// The database file is placed in the platform-appropriate data directory:
    /// - Linux:   `~/.local/share/liberte/liberte.db`
    /// - macOS:   `~/Library/Application Support/com.liberte.liberte/liberte.db`
    /// - Windows: `{FOLDERID_RoamingAppData}\liberte\liberte\data\liberte.db`
    ///
    /// # Arguments
    /// * `_db_key` -- reserved for future SQLCipher support.
    pub fn new(_db_key: &[u8; 32]) -> Result<Self> {
        let project_dirs =
            ProjectDirs::from("com", "liberte", "liberte").ok_or(StoreError::NoDataDir)?;

        let data_dir = project_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        let db_path = data_dir.join("liberte.db");

        tracing::info!(path = %db_path.display(), "opening database");

        Self::open_at(&db_path, _db_key)
    }

    /// Open (or create) a database at an explicit path.
    ///
    /// This is useful for tests and for embedding the store inside custom
    /// directory layouts.
    pub fn open_at(path: &Path, _db_key: &[u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Recommended SQLite settings.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        // Run schema migrations.
        migrations::run_migrations(&conn)?;

        Ok(Self { conn })
    }

    /// Return a reference to the underlying `rusqlite::Connection`.
    ///
    /// Callers should prefer the typed CRUD helpers, but direct access is
    /// occasionally needed for transactions or ad-hoc queries.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Return a mutable reference to the underlying connection.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Return the filesystem path of the open database (if any).
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
