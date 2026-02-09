pub mod v001_initial;
pub mod v002_channel_keys;

use rusqlite::Connection;

use crate::error::{Result, StoreError};

const CURRENT_VERSION: u32 = 2;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

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

    Ok(())
}
