use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{ChannelId, ConnectionMode, ServerId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    ChatMessage(ChatMessage),
    FileOffer(FileOffer),
    FileAccept(FileAcceptance),
    FileChunk(FileChunk),
    Signal(SignalMessage),
    PeerStatus(PeerStatus),
    ChannelInvite(ChannelInvite),
    PremiumAuth(PremiumAuth),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub encrypted_content: Vec<u8>, // nonce || ciphertext
    pub timestamp: DateTime<Utc>,
    pub message_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffer {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub file_id: uuid::Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub file_hash: [u8; 32], // BLAKE3 hash for integrity
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcceptance {
    pub file_id: uuid::Uuid,
    pub accepter: UserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub file_id: uuid::Uuid,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    pub sender: UserId,
    pub target: UserId,
    pub channel_id: ChannelId,
    pub signal_type: SignalType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Offer(String),
    Answer(String),
    IceCandidate(String),
    Hangup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatus {
    pub user_id: UserId,
    pub online: bool,
    pub connection_mode: ConnectionMode,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInvite {
    pub inviter: UserId,
    pub channel_id: ChannelId,
    pub server_id: Option<ServerId>,
    pub channel_name: String,
    pub encrypted_channel_secret: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumAuth {
    pub user_id: UserId,
    pub token: Vec<u8>,
}

impl WireMessage {
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_message_roundtrip() {
        let msg = WireMessage::ChatMessage(ChatMessage {
            sender: UserId([42u8; 32]),
            channel_id: ChannelId(uuid::Uuid::new_v4()),
            encrypted_content: vec![1, 2, 3, 4, 5],
            timestamp: Utc::now(),
            message_id: uuid::Uuid::new_v4(),
        });

        let bytes = msg.to_bytes().unwrap();
        let restored = WireMessage::from_bytes(&bytes).unwrap();

        if let (WireMessage::ChatMessage(orig), WireMessage::ChatMessage(rest)) = (&msg, &restored)
        {
            assert_eq!(orig.sender, rest.sender);
            assert_eq!(orig.encrypted_content, rest.encrypted_content);
        } else {
            panic!("Message type mismatch");
        }
    }
}
