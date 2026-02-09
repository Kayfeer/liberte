use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{ChannelId, ConnectionMode, ServerId, UserId};

/// All wire protocol messages exchanged between peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    /// Encrypted chat message
    ChatMessage(ChatMessage),

    /// File transfer offer
    FileOffer(FileOffer),

    /// File transfer acceptance
    FileAccept(FileAcceptance),

    /// File chunk (for P2P direct transfer)
    FileChunk(FileChunk),

    /// WebRTC signaling (SDP offer/answer/ICE candidates)
    Signal(SignalMessage),

    /// Peer status update
    PeerStatus(PeerStatus),

    /// Channel invite
    ChannelInvite(ChannelInvite),

    /// Premium token presentation (for SFU/relay access)
    PremiumAuth(PremiumAuth),
}

/// An encrypted chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Sender's public key
    pub sender: UserId,
    /// Target channel
    pub channel_id: ChannelId,
    /// Encrypted content (XChaCha20-Poly1305: nonce || ciphertext)
    pub encrypted_content: Vec<u8>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Message UUID for deduplication
    pub message_id: uuid::Uuid,
}

/// Offer to transfer a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffer {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub file_id: uuid::Uuid,
    pub file_name: String,
    pub file_size: u64,
    /// BLAKE3 hash of the unencrypted file for integrity verification
    pub file_hash: [u8; 32],
    pub timestamp: DateTime<Utc>,
}

/// Accept a file transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAcceptance {
    pub file_id: uuid::Uuid,
    pub accepter: UserId,
}

/// A chunk of file data during P2P transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub file_id: uuid::Uuid,
    pub chunk_index: u32,
    pub total_chunks: u32,
    /// Encrypted chunk data
    pub data: Vec<u8>,
}

/// WebRTC signaling message for audio/video calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    pub sender: UserId,
    pub target: UserId,
    pub channel_id: ChannelId,
    pub signal_type: SignalType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    /// SDP Offer
    Offer(String),
    /// SDP Answer
    Answer(String),
    /// ICE Candidate
    IceCandidate(String),
    /// Call ended
    Hangup,
}

/// Peer online/offline status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatus {
    pub user_id: UserId,
    pub online: bool,
    pub connection_mode: ConnectionMode,
    pub timestamp: DateTime<Utc>,
}

/// Invitation to join a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInvite {
    pub inviter: UserId,
    pub channel_id: ChannelId,
    pub server_id: Option<ServerId>,
    pub channel_name: String,
    /// Encrypted channel shared secret (encrypted with recipient's key)
    pub encrypted_channel_secret: Vec<u8>,
}

/// Premium authentication token for SFU/relay access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumAuth {
    pub user_id: UserId,
    pub token: Vec<u8>,
}

impl WireMessage {
    /// Serialize to binary (bincode)
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize from binary
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
