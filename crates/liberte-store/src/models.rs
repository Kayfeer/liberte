use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub pubkey: [u8; 32],
    pub display_name: Option<String>,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub server_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub sender_pubkey: [u8; 32],
    pub encrypted_content: Vec<u8>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub owner_pubkey: [u8; 32],
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Blob {
    pub id: Uuid,
    pub file_name: String,
    pub file_size: i64,
    pub blake3_hash: String,
    pub is_uploaded: bool,
    pub local_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reaction {
    pub id: Uuid,
    pub message_id: Uuid,
    pub channel_id: Uuid,
    pub user_pubkey: String,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}
