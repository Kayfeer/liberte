//! Database migration runner.
//!
//! Migrations are executed in order on every [`Database::new`] / [`Database::open_at`]
//! call.  Each migration is guarded by a `user_version` pragma so it runs
//! exactly once.

pub mod v001_initial;

use rusqlite::Connection;

use crate::error::{Result, StoreError};

/// Current schema version.  Bump this and add a new migration module whenever
/// the schema changes.
const CURRENT_VERSION: u32 = 1;

/// Run all pending migrations against the open connection.
///
/// The function reads `PRAGMA user_version` to determine which migrations have
/// already been applied, then executes any outstanding ones in order.
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
