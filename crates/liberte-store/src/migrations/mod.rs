pub mod v001_initial;

use rusqlite::Connection;

use crate::error::{Result, StoreError};

const CURRENT_VERSION: u32 = 1;

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

    // Future migrations would be added here:
    // if current < 2 {
    //     v002_xxx::up(conn)?;
    //     conn.pragma_update(None, "user_version", 2)?;
    // }

    Ok(())
}
