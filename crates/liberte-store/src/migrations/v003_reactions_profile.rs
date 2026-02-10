use rusqlite::Connection;

const UP_SQL: &str = r#"
-- Add bio and status columns to users table
ALTER TABLE users ADD COLUMN bio TEXT;
ALTER TABLE users ADD COLUMN status TEXT NOT NULL DEFAULT 'online';

-- Reactions table
CREATE TABLE IF NOT EXISTS reactions (
    id         TEXT PRIMARY KEY NOT NULL,   -- UUID v4
    message_id TEXT NOT NULL,              -- FK -> messages(id)
    channel_id TEXT NOT NULL,              -- FK -> channels(id)
    user_pubkey TEXT NOT NULL,             -- hex-encoded pubkey
    emoji      TEXT NOT NULL,              -- emoji character(s)
    created_at TEXT NOT NULL,              -- ISO-8601

    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reactions_message ON reactions(message_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reactions_unique ON reactions(message_id, user_pubkey, emoji);
"#;

pub fn up(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(UP_SQL)
}
