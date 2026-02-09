//! Domain model structs persisted in the local SQLCipher database.
//!
//! Every struct derives `Serialize` and `Deserialize` so it can be handed
//! directly to the UI layer over IPC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

/// A known user identity.  The primary key is the 32-byte Ed25519 public key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    /// Ed25519 public key (32 bytes), stored as hex in SQLite.
    pub pubkey: [u8; 32],
    /// Optional human-readable display name.
    pub display_name: Option<String>,
    /// Optional BLAKE3 hash of the avatar image blob.
    pub avatar_hash: Option<String>,
    /// Timestamp when this user was first seen / created locally.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Channel
// ---------------------------------------------------------------------------

/// A conversation channel (DM or group).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    /// Unique channel identifier.
    pub id: Uuid,
    /// Human-readable channel name.
    pub name: String,
    /// If this channel belongs to a server, the server's UUID.
    pub server_id: Option<Uuid>,
    /// When the channel was created locally.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A single chat message.  The content is always stored encrypted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    /// Unique message identifier.
    pub id: Uuid,
    /// The channel this message belongs to.
    pub channel_id: Uuid,
    /// Ed25519 public key of the sender (32 bytes), stored as hex.
    pub sender_pubkey: [u8; 32],
    /// Encrypted message content (opaque bytes).
    pub encrypted_content: Vec<u8>,
    /// When the message was sent (as reported by the sender).
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Server (guild)
// ---------------------------------------------------------------------------

/// A server (also called "guild") that groups channels and members.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Server {
    /// Unique server identifier.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Ed25519 public key of the owner (32 bytes), stored as hex.
    pub owner_pubkey: [u8; 32],
    /// When the server was created locally.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Blob (file metadata)
// ---------------------------------------------------------------------------

/// Metadata for a file stored locally (and optionally uploaded).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Blob {
    /// Unique blob identifier.
    pub id: Uuid,
    /// Original file name.
    pub file_name: String,
    /// File size in bytes.
    pub file_size: i64,
    /// BLAKE3 content hash (hex string).
    pub blake3_hash: String,
    /// Whether the blob has been uploaded to a remote peer / server.
    pub is_uploaded: bool,
    /// Absolute path on disk where the file is stored locally.
    pub local_path: String,
    /// When this blob record was created.
    pub created_at: DateTime<Utc>,
}
