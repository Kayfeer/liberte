use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

// Token signed by payment server, client presents it to relay/SFU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumToken {
    pub user_pubkey: [u8; 32],
    pub valid_until: DateTime<Utc>,
    pub signature: Vec<u8>,
}

/// Returns the payment server Ed25519 public key.
///
/// In production, set the `PAYMENT_SERVER_PUBKEY` environment variable to the
/// 64-character hex-encoded public key. When unset, falls back to a dev key
/// that rejects all tokens.
pub fn payment_server_pubkey() -> [u8; 32] {
    if let Ok(hex_str) = std::env::var("PAYMENT_SERVER_PUBKEY") {
        if let Ok(bytes) = hex::decode(&hex_str) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return key;
            }
        }
        eprintln!("WARNING: PAYMENT_SERVER_PUBKEY env var is invalid, using default");
    }
    // Dev fallback — always rejects tokens
    [0u8; 32]
}

pub fn check_premium_status(token: &PremiumToken) -> bool {
    let key = payment_server_pubkey();
    check_premium_status_with_key(token, &key)
}

pub fn check_premium_status_with_key(token: &PremiumToken, server_pubkey: &[u8; 32]) -> bool {
    if Utc::now() > token.valid_until {
        return false;
    }

    let Ok(verifying_key) = VerifyingKey::from_bytes(server_pubkey) else {
        return false;
    };

    // payload = user_pubkey || valid_until (rfc3339)
    let mut payload = Vec::new();
    payload.extend_from_slice(&token.user_pubkey);
    payload.extend_from_slice(token.valid_until.to_rfc3339().as_bytes());

    let Ok(signature) = Signature::from_slice(&token.signature) else {
        return false;
    };

    verifying_key.verify(&payload, &signature).is_ok()
}

pub fn create_premium_token(
    user_pubkey: &[u8; 32],
    valid_until: DateTime<Utc>,
    server_signing_key: &ed25519_dalek::SigningKey,
) -> PremiumToken {
    use ed25519_dalek::Signer;

    let mut payload = Vec::new();
    payload.extend_from_slice(user_pubkey);
    payload.extend_from_slice(valid_until.to_rfc3339().as_bytes());

    let signature = server_signing_key.sign(&payload);

    PremiumToken {
        user_pubkey: *user_pubkey,
        valid_until,
        signature: signature.to_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_premium_token_valid() {
        let server_key = SigningKey::generate(&mut OsRng);
        let server_pubkey = server_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() + Duration::days(30),
            &server_key,
        );

        assert!(check_premium_status_with_key(&token, &server_pubkey));
    }

    #[test]
    fn test_premium_token_expired() {
        let server_key = SigningKey::generate(&mut OsRng);
        let server_pubkey = server_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() - Duration::days(1),
            &server_key,
        );

        assert!(!check_premium_status_with_key(&token, &server_pubkey));
    }

    #[test]
    fn test_premium_token_wrong_server_key() {
        let server_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let wrong_pubkey = wrong_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() + Duration::days(30),
            &server_key,
        );

        assert!(!check_premium_status_with_key(&token, &wrong_pubkey));
    }
}
