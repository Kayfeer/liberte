pub mod v001_initial;
pub mod v002_channel_keys;
pub mod v003_reactions_profile;
pub mod v004_peer_addresses;

use rusqlite::Connection;

use crate::error::{Result, StoreError};

const CURRENT_VERSION: u32 = 4;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > CURRENT_VERSION {
        return Err(StoreError::Migration(
            "Database was created by a newer version of Liberté".into(),
        ));
    }
    let transaction = conn.unchecked_transaction()?;

    tracing::info!(
        current_version = current,
        target_version = CURRENT_VERSION,
        "checking database migrations"
    );

    if current < 1 {
        tracing::info!("applying migration v001_initial");
        v001_initial::up(conn).map_err(|e| StoreError::Migration(e.to_string()))?;
        conn.pragma_update(None, "user_version", 1)?;
    }

    if current < 2 {
        tracing::info!("applying migration v002_channel_keys");
        v002_channel_keys::up(conn).map_err(|e| StoreError::Migration(e.to_string()))?;
        conn.pragma_update(None, "user_version", 2)?;
    }

    if current < 3 {
        tracing::info!("applying migration v003_reactions_profile");
        v003_reactions_profile::up(conn).map_err(|e| StoreError::Migration(e.to_string()))?;
        conn.pragma_update(None, "user_version", 3)?;
    }

    if current < 4 {
        tracing::info!("applying migration v004_peer_addresses");
        v004_peer_addresses::up(conn).map_err(|e| StoreError::Migration(e.to_string()))?;
        conn.pragma_update(None, "user_version", 4)?;
    }

    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_upgrade_rolls_back_schema_and_version() {
        let connection = Connection::open_in_memory().unwrap();
        v001_initial::up(&connection).unwrap();
        v002_channel_keys::up(&connection).unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        // An incompatible existing table causes the later v3 index creation to
        // fail after both ALTER TABLE statements would otherwise have committed.
        connection
            .execute_batch("CREATE TABLE reactions (id TEXT PRIMARY KEY);")
            .unwrap();
        assert!(run_migrations(&connection).is_err());
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        let mut statement = connection.prepare("PRAGMA table_info(users)").unwrap();
        let names = statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(!names.iter().any(|name| name == "bio" || name == "status"));
    }

    #[test]
    fn peer_address_upgrade_preserves_existing_data_and_is_idempotent() {
        let connection = Connection::open_in_memory().unwrap();
        v001_initial::up(&connection).unwrap();
        v002_channel_keys::up(&connection).unwrap();
        v003_reactions_profile::up(&connection).unwrap();
        connection.pragma_update(None, "user_version", 3).unwrap();
        connection.execute("INSERT INTO channels (id,name,created_at) VALUES ('channel','existing history','2026-01-01T00:00:00Z')", []).unwrap();
        run_migrations(&connection).unwrap();
        connection.execute("INSERT INTO peer_addresses (peer_id,addresses_json,updated_at) VALUES ('peer','[]',1)", []).unwrap();
        run_migrations(&connection).unwrap();
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);
        let channels: i64 = connection
            .query_row("SELECT COUNT(*) FROM channels", [], |row| row.get(0))
            .unwrap();
        let peers: i64 = connection
            .query_row("SELECT COUNT(*) FROM peer_addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!((channels, peers), (1, 1));
    }

    #[test]
    fn future_database_version_is_not_modified() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "user_version", CURRENT_VERSION + 1)
            .unwrap();
        assert!(run_migrations(&connection).is_err());
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION + 1);
    }
}
