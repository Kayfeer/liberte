use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::Identity;

const INVITE_DURATION_SECS: i64 = 300; // 5 minutes

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub channel_id: Uuid,
    pub channel_name: String,
    pub inviter_pubkey: [u8; 32],
    pub channel_key: [u8; 32],
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteToken {
    pub payload: InvitePayload,
    pub signature: Vec<u8>,
}

impl InviteToken {
    /// Create a new signed invite token.
    pub fn create(
        identity: &Identity,
        channel_id: Uuid,
        channel_name: String,
        channel_key: [u8; 32],
    ) -> Self {
        let now = Utc::now();
        let payload = InvitePayload {
            channel_id,
            channel_name,
            inviter_pubkey: identity.public_key_bytes(),
            channel_key,
            created_at: now,
            expires_at: now + Duration::seconds(INVITE_DURATION_SECS),
        };

        let payload_bytes = bincode::serialize(&payload).expect("payload serialization");
        let signature = identity.sign(&payload_bytes);

        Self {
            payload,
            signature: signature.to_bytes().to_vec(),
        }
    }

    /// Encode the token as a base64url string (copiable code).
    pub fn encode(&self) -> String {
        let bytes = bincode::serialize(self).expect("token serialization");
        base64_url_encode(&bytes)
    }

    /// Decode a base64url string back into an InviteToken.
    pub fn decode(code: &str) -> Result<Self, InviteError> {
        let bytes = base64_url_decode(code)?;
        bincode::deserialize(&bytes).map_err(|_| InviteError::InvalidFormat)
    }

    /// Verify the token's signature and expiry.
    pub fn verify(&self) -> Result<(), InviteError> {
        // Check expiry
        if Utc::now() > self.payload.expires_at {
            return Err(InviteError::Expired);
        }

        // Verify Ed25519 signature
        let payload_bytes =
            bincode::serialize(&self.payload).map_err(|_| InviteError::InvalidFormat)?;

        let signature = Signature::from_slice(&self.signature)
            .map_err(|_| InviteError::InvalidSignature)?;

        let verifying_key = VerifyingKey::from_bytes(&self.payload.inviter_pubkey)
            .map_err(|_| InviteError::InvalidSignature)?;

        verifying_key
            .verify(&payload_bytes, &signature)
            .map_err(|_| InviteError::InvalidSignature)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    #[error("Invalid invite format")]
    InvalidFormat,

    #[error("Invite has expired")]
    Expired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Base64 decode error")]
    Base64Decode,
}

fn base64_url_encode(data: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(data)
}

fn base64_url_decode(s: &str) -> Result<Vec<u8>, InviteError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD
        .decode(s.trim())
        .map_err(|_| InviteError::Base64Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invite_roundtrip() {
        let identity = Identity::generate();
        let channel_id = Uuid::new_v4();
        let channel_key = [0xABu8; 32];

        let token = InviteToken::create(
            &identity,
            channel_id,
            "test-channel".to_string(),
            channel_key,
        );

        let code = token.encode();
        let decoded = InviteToken::decode(&code).expect("decode should work");
        decoded.verify().expect("verify should pass");

        assert_eq!(decoded.payload.channel_id, channel_id);
        assert_eq!(decoded.payload.channel_name, "test-channel");
        assert_eq!(decoded.payload.channel_key, channel_key);
        assert_eq!(decoded.payload.inviter_pubkey, identity.public_key_bytes());
    }

    #[test]
    fn test_invite_tampered_fails() {
        let identity = Identity::generate();
        let token = InviteToken::create(
            &identity,
            Uuid::new_v4(),
            "channel".to_string(),
            [0u8; 32],
        );

        let mut bad_token = token;
        bad_token.payload.channel_name = "hacked".to_string();
        assert!(bad_token.verify().is_err());
    }
}
