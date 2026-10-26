use rusqlite::Connection;

/// Reconstructible connectivity cache; it does not contain channel keys or history.
pub fn up(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE peer_addresses (
            peer_id TEXT PRIMARY KEY NOT NULL,
            addresses_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    )
}
