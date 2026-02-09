//! v001 -- Initial schema creation.
//!
//! Creates the five core tables: `users`, `channels`, `messages`, `servers`,
//! and `blobs`.

use rusqlite::Connection;

/// SQL executed when upgrading from version 0 to version 1.
const UP_SQL: &str = r#"
-- ----------------------------------------------------------------
-- Users
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    pubkey       TEXT PRIMARY KEY NOT NULL,   -- hex-encoded 32-byte Ed25519 pubkey
    display_name TEXT,
    avatar_hash  TEXT,
    created_at   TEXT NOT NULL                -- ISO-8601 / RFC-3339
);

-- ----------------------------------------------------------------
-- Servers (guilds)
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS servers (
    id           TEXT PRIMARY KEY NOT NULL,   -- UUID v4
    name         TEXT NOT NULL,
    owner_pubkey TEXT NOT NULL,               -- hex-encoded pubkey
    created_at   TEXT NOT NULL
);

-- ----------------------------------------------------------------
-- Channels
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS channels (
    id         TEXT PRIMARY KEY NOT NULL,     -- UUID v4
    name       TEXT NOT NULL,
    server_id  TEXT,                          -- nullable FK -> servers(id)
    created_at TEXT NOT NULL,

    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channels_server_id ON channels(server_id);

-- ----------------------------------------------------------------
-- Messages
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS messages (
    id                TEXT PRIMARY KEY NOT NULL,  -- UUID v4
    channel_id        TEXT NOT NULL,              -- FK -> channels(id)
    sender_pubkey     TEXT NOT NULL,              -- hex-encoded pubkey
    encrypted_content BLOB NOT NULL,              -- opaque ciphertext
    timestamp         TEXT NOT NULL,              -- ISO-8601

    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_messages_channel_ts
    ON messages(channel_id, timestamp DESC);

-- ----------------------------------------------------------------
-- Blobs (file metadata)
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS blobs (
    id          TEXT PRIMARY KEY NOT NULL,    -- UUID v4
    file_name   TEXT NOT NULL,
    file_size   INTEGER NOT NULL,
    blake3_hash TEXT NOT NULL,
    is_uploaded INTEGER NOT NULL DEFAULT 0,   -- boolean 0/1
    local_path  TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
"#;

/// Apply the initial migration.
pub fn up(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(UP_SQL)
}
