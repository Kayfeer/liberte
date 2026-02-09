use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// A premium token signed by the payment server.
/// The client presents this to the relay/SFU to prove payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumToken {
    /// The user's Ed25519 public key
    pub user_pubkey: [u8; 32],
    /// Expiration timestamp
    pub valid_until: DateTime<Utc>,
    /// Ed25519 signature by the payment server
    pub signature: Vec<u8>,
}

/// The payment server's public key (placeholder - replace with real key in production).
/// This is compiled into the binary for verification.
pub const PAYMENT_SERVER_PUBKEY: [u8; 32] = [0u8; 32];

/// Verify that a user has valid premium status.
///
/// The token contains: user's pubkey + expiration, signed by the payment server.
/// This function checks both expiration and signature validity.
pub fn check_premium_status(token: &PremiumToken) -> bool {
    check_premium_status_with_key(token, &PAYMENT_SERVER_PUBKEY)
}

/// Verify premium status using a specific payment server public key.
/// Useful for testing and when the payment server key is configurable.
pub fn check_premium_status_with_key(token: &PremiumToken, server_pubkey: &[u8; 32]) -> bool {
    // 1. Check expiration
    if Utc::now() > token.valid_until {
        return false;
    }

    // 2. Verify signature
    let Ok(verifying_key) = VerifyingKey::from_bytes(server_pubkey) else {
        return false;
    };

    // Construct the signed payload: user_pubkey || valid_until as RFC3339 bytes
    let mut payload = Vec::new();
    payload.extend_from_slice(&token.user_pubkey);
    payload.extend_from_slice(token.valid_until.to_rfc3339().as_bytes());

    let Ok(signature) = Signature::from_slice(&token.signature) else {
        return false;
    };

    verifying_key.verify(&payload, &signature).is_ok()
}

/// Create a premium token (used by payment server)
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
            Utc::now() - Duration::days(1), // Already expired
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

        // Verify with wrong key should fail
        assert!(!check_premium_status_with_key(&token, &wrong_pubkey));
    }
}
