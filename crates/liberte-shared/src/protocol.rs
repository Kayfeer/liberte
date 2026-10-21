use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use bincode::Options;
use ed25519_dalek::{Signature, VerifyingKey};
use crate::{crypto, identity::Identity, constants::MAX_MESSAGE_SIZE};

use crate::types::{ChannelId, ConnectionMode, ServerId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    ChatMessage(ChatMessage),
    FileOffer(FileOffer),
    FileAccept(FileAcceptance),
    FileChunk(FileChunk),
    FileComplete(FileCompletion),
    Signal(SignalMessage),
    PeerStatus(PeerStatus),
    ChannelInvite(ChannelInvite),
    PremiumAuth(PremiumAuth),
    TypingIndicator(TypingIndicator),
    StatusUpdate(StatusUpdate),
    MessageReaction(MessageReactionMsg),
    VoiceFrame(VoiceFrame),
    VoiceEvent(VoiceEvent),
    ProfileUpdate(ProfileUpdate),
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
pub struct FileCompletion {
    pub file_id: uuid::Uuid,
    pub receiver: UserId,
    pub channel_id: ChannelId,
    pub file_hash: [u8; 32],
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingIndicator {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub sender_display_name: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub user_id: UserId,
    pub status: String, // "online", "dnd", "idle", "invisible"
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUpdate {
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReactionMsg {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub message_id: uuid::Uuid,
    pub emoji: String,
    pub action: ReactionAction,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactionAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceFrame {
    pub sender: UserId,
    pub channel_id: ChannelId,
    pub sequence: u32,
    pub audio_data: Vec<u8>, // 16-bit PCM, 16kHz mono, 20ms = 640 bytes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceEvent {
    pub user_id: UserId,
    pub channel_id: ChannelId,
    pub event_type: VoiceEventType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoiceEventType {
    Join,
    Leave,
    Mute,
    Unmute,
}

impl WireMessage {
    /// Authenticated protocol v2. Legacy plaintext wire messages are never accepted here.
    pub fn seal(&self, channel_id: ChannelId, key: &[u8; 32], identity: &Identity) -> Result<Vec<u8>, String> {
        self.validate(&identity.user_id(), &channel_id)?;
        let body = EnvelopeBody {
            domain: "liberte-envelope-v2".to_string(),
            channel_id,
            author: identity.user_id(),
            id: uuid::Uuid::new_v4(),
            issued_at_ms: Utc::now().timestamp_millis(),
            message: self.clone(),
        };
        let signed = codec().serialize(&body).map_err(|e| e.to_string())?;
        let envelope = SignedEnvelope { body, signature: identity.sign(&signed).to_bytes().to_vec() };
        let plaintext = codec().serialize(&envelope).map_err(|e| e.to_string())?;
        let encrypted = crypto::encrypt(key, &plaintext).map_err(|e| e.to_string())?;
        if encrypted.len() > MAX_MESSAGE_SIZE - 1024 { return Err("Message exceeds network limit".into()); }
        Ok(encrypted)
    }

    pub fn open(data: &[u8], expected_channel: ChannelId, key: &[u8; 32]) -> Result<(UserId, WireMessage), String> {
        let verified = Self::open_authenticated(data, expected_channel, key)?;
        Ok((verified.author, verified.message))
    }

    pub fn open_authenticated(data: &[u8], expected_channel: ChannelId, key: &[u8; 32]) -> Result<VerifiedMessage, String> {
        if data.len() > MAX_MESSAGE_SIZE { return Err("Message exceeds network limit".into()); }
        let plaintext = crypto::decrypt(key, data).map_err(|_| "Invalid encrypted envelope".to_string())?;
        let envelope: SignedEnvelope = codec().deserialize(&plaintext).map_err(|_| "Invalid envelope format".to_string())?;
        let body = &envelope.body;
        if body.domain != "liberte-envelope-v2" || body.channel_id != expected_channel {
            return Err("Envelope protocol or channel mismatch".into());
        }
        let age = Utc::now().timestamp_millis().saturating_sub(body.issued_at_ms);
        if !(-30_000..=300_000).contains(&age) { return Err("Envelope expired or from future".into()); }
        let signed = codec().serialize(body).map_err(|e| e.to_string())?;
        let public = VerifyingKey::from_bytes(&body.author.0).map_err(|_| "Invalid author key".to_string())?;
        let signature = Signature::from_slice(&envelope.signature).map_err(|_| "Invalid signature".to_string())?;
        public.verify_strict(&signed, &signature).map_err(|_| "Invalid signature".to_string())?;
        body.message.validate(&body.author, &expected_channel)?;
        Ok(VerifiedMessage { author: body.author.clone(), id: body.id, issued_at_ms: body.issued_at_ms, message: body.message.clone() })
    }

    fn validate(&self, author: &UserId, channel: &ChannelId) -> Result<(), String> {
        let (sender, inner_channel) = match self {
            Self::ChatMessage(v) => {
                if v.encrypted_content.len() > 65_576 { return Err("Chat message too large".into()); }
                (&v.sender, Some(&v.channel_id))
            }
            Self::FileOffer(v) => {
                if v.file_name.is_empty() || v.file_name.len() > 255 || v.file_size > crate::constants::MAX_FILE_SIZE as u64 { return Err("Invalid file offer".into()); }
                (&v.sender, Some(&v.channel_id))
            }
            Self::FileAccept(v) => (&v.accepter, None),
            Self::FileComplete(v) => (&v.receiver, Some(&v.channel_id)),
            Self::FileChunk(v) => {
                if v.data.len() > 65_536 || v.total_chunks == 0 || v.chunk_index >= v.total_chunks || v.total_chunks > 1600 { return Err("Invalid file chunk".into()); }
                (author, None)
            }
            Self::Signal(v) => (&v.sender, Some(&v.channel_id)),
            Self::PeerStatus(v) => (&v.user_id, None),
            Self::ChannelInvite(v) => (&v.inviter, Some(&v.channel_id)),
            Self::PremiumAuth(v) => (&v.user_id, None),
            Self::TypingIndicator(v) => {
                if v.sender_display_name.as_ref().is_some_and(|s| s.len() > 128) { return Err("Display name too long".into()); }
                (&v.sender, Some(&v.channel_id))
            }
            Self::StatusUpdate(v) => {
                if !["online", "dnd", "idle", "invisible"].contains(&v.status.as_str()) { return Err("Invalid status".into()); }
                (&v.user_id, None)
            }
            Self::MessageReaction(v) => {
                if v.emoji.is_empty() || v.emoji.len() > 32 { return Err("Invalid reaction".into()); }
                (&v.sender, Some(&v.channel_id))
            }
            Self::VoiceFrame(v) => {
                if v.audio_data.len() != 640 { return Err("Invalid voice frame size".into()); }
                (&v.sender, Some(&v.channel_id))
            }
            Self::VoiceEvent(v) => (&v.user_id, Some(&v.channel_id)),
            Self::ProfileUpdate(v) => {
                if v.display_name.as_ref().is_some_and(|s| s.len() > 128)
                    || v.bio.as_ref().is_some_and(|s| s.len() > 2000)
                    || !["online", "dnd", "idle", "invisible"].contains(&v.status.as_str()) {
                    return Err("Invalid profile update".into());
                }
                (&v.user_id, None)
            }
        };
        if sender != author { return Err("Payload author does not match signer".into()); }
        if inner_channel.is_some_and(|c| c != channel) { return Err("Payload channel mismatch".into()); }
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new().with_fixint_encoding().with_limit(MAX_MESSAGE_SIZE as u64).reject_trailing_bytes()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvelopeBody {
    domain: String,
    channel_id: ChannelId,
    author: UserId,
    id: uuid::Uuid,
    issued_at_ms: i64,
    message: WireMessage,
}

#[derive(Serialize, Deserialize)]
struct SignedEnvelope { body: EnvelopeBody, signature: Vec<u8> }

pub struct VerifiedMessage {
    pub author: UserId,
    pub id: uuid::Uuid,
    pub issued_at_ms: i64,
    pub message: WireMessage,
}

/// Per-client replay guard. Full capacity rejects new messages instead of evicting live entries.
#[derive(Default)]
pub struct ReplayGuard { seen: std::collections::HashMap<uuid::Uuid, i64>, last_pruned: i64 }

impl ReplayGuard {
    pub fn accept(&mut self, message: &VerifiedMessage) -> bool {
        let now = Utc::now().timestamp_millis();
        if now.saturating_sub(self.last_pruned) >= 30_000 {
            self.seen.retain(|_, issued| now.saturating_sub(*issued) <= 330_000);
            self.last_pruned = now;
        }
        if self.seen.contains_key(&message.id) || self.seen.len() >= 65_536 { return false; }
        self.seen.insert(message.id, message.issued_at_ms);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authenticated_fixture() -> (Identity, ChannelId, [u8; 32], WireMessage) {
        let identity = Identity::generate();
        let channel = ChannelId::new();
        let key = crypto::generate_symmetric_key();
        let msg = WireMessage::ChatMessage(ChatMessage {
            sender: identity.user_id(), channel_id: channel.clone(),
            encrypted_content: crypto::encrypt(&key, b"hello").unwrap(),
            timestamp: Utc::now(), message_id: uuid::Uuid::new_v4(),
        });
        (identity, channel, key, msg)
    }

    #[test]
    fn authenticated_envelope_rejects_plaintext_wrong_channel_and_wrong_key() {
        let (id, channel, key, msg) = authenticated_fixture();
        let bytes = msg.seal(channel.clone(), &key, &id).unwrap();
        let (author, _) = WireMessage::open(&bytes, channel.clone(), &key).unwrap();
        assert_eq!(author, id.user_id());
        assert!(WireMessage::open(&msg.to_bytes().unwrap(), channel.clone(), &key).is_err());
        assert!(WireMessage::open(&bytes, ChannelId::new(), &key).is_err());
        assert!(WireMessage::open(&bytes, channel, &[0; 32]).is_err());
    }

    #[test]
    fn member_cannot_forge_another_author_or_modify_signed_metadata() {
        let (id, channel, key, msg) = authenticated_fixture();
        let attacker = Identity::generate();
        assert!(msg.seal(channel.clone(), &key, &attacker).is_err());
        let bytes = msg.seal(channel.clone(), &key, &id).unwrap();
        let plaintext = crypto::decrypt(&key, &bytes).unwrap();
        let mut envelope: SignedEnvelope = codec().deserialize(&plaintext).unwrap();
        envelope.body.id = uuid::Uuid::new_v4();
        let modified = crypto::encrypt(&key, &codec().serialize(&envelope).unwrap()).unwrap();
        assert!(WireMessage::open(&modified, channel, &key).is_err());
    }

    #[test]
    fn valid_member_signature_cannot_claim_a_different_payload_author() {
        let (id, channel, key, msg) = authenticated_fixture();
        let bytes = msg.seal(channel.clone(), &key, &id).unwrap();
        let mut envelope: SignedEnvelope = codec().deserialize(&crypto::decrypt(&key, &bytes).unwrap()).unwrap();
        if let WireMessage::ChatMessage(ref mut chat) = envelope.body.message {
            chat.sender = Identity::generate().user_id();
        }
        envelope.signature = id.sign(&codec().serialize(&envelope.body).unwrap()).to_bytes().to_vec();
        let forged = crypto::encrypt(&key, &codec().serialize(&envelope).unwrap()).unwrap();
        assert!(WireMessage::open(&forged, channel, &key).is_err());
    }

    #[test]
    fn replay_is_rejected_even_after_reencryption_with_a_new_nonce() {
        let (id, channel, key, msg) = authenticated_fixture();
        let bytes = msg.seal(channel.clone(), &key, &id).unwrap();
        let original = WireMessage::open_authenticated(&bytes, channel.clone(), &key).unwrap();
        let reencrypted = crypto::encrypt(&key, &crypto::decrypt(&key, &bytes).unwrap()).unwrap();
        let replay = WireMessage::open_authenticated(&reencrypted, channel, &key).unwrap();
        let mut guard = ReplayGuard::default();
        assert!(guard.accept(&original));
        assert!(!guard.accept(&replay));
    }

    #[test]
    fn expired_signed_envelope_is_rejected() {
        let (id, channel, key, msg) = authenticated_fixture();
        let bytes = msg.seal(channel.clone(), &key, &id).unwrap();
        let mut envelope: SignedEnvelope = codec().deserialize(&crypto::decrypt(&key, &bytes).unwrap()).unwrap();
        envelope.body.issued_at_ms -= 600_000;
        envelope.signature = id.sign(&codec().serialize(&envelope.body).unwrap()).to_bytes().to_vec();
        let expired = crypto::encrypt(&key, &codec().serialize(&envelope).unwrap()).unwrap();
        assert!(WireMessage::open(&expired, channel, &key).is_err());
    }

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

