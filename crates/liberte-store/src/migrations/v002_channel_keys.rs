use rusqlite::Connection;

const UP_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS channel_keys (
    channel_id TEXT PRIMARY KEY NOT NULL,     -- UUID v4, FK -> channels(id)
    key_hex    TEXT NOT NULL,                 -- hex-encoded 32-byte symmetric key
    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);
"#;

pub fn up(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(UP_SQL)
}
