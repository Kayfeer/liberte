use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::IdentityError;
use crate::types::UserId;

/// A user's cryptographic identity based on Ed25519.
/// The public key serves as the user ID. No email, no phone number.
#[derive(Clone)]
pub struct Identity {
    signing_key: SigningKey,
}

/// Serializable format for storing/exporting identity
#[derive(Serialize, Deserialize)]
pub struct IdentityExport {
    pub secret_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl Identity {
    /// Generate a new random identity
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Restore identity from secret key bytes
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(secret);
        Self { signing_key }
    }

    /// Restore identity from a serialized export
    pub fn from_export(export: &IdentityExport) -> Self {
        Self::from_secret_bytes(&export.secret_key)
    }

    /// Get the user ID (public key)
    pub fn user_id(&self) -> UserId {
        UserId(self.signing_key.verifying_key().to_bytes())
    }

    /// Get the raw public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Get the raw secret key bytes
    pub fn secret_bytes(&self) -> &[u8; 32] {
        self.signing_key.as_bytes()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Get the verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Export identity for serialization
    pub fn to_export(&self) -> IdentityExport {
        IdentityExport {
            secret_key: *self.signing_key.as_bytes(),
            public_key: self.signing_key.verifying_key().to_bytes(),
        }
    }

    /// Derive a database encryption key from the identity using BLAKE3
    pub fn derive_db_key(&self) -> [u8; 32] {
        let mut hasher =
            blake3::Hasher::new_derive_key(crate::constants::KDF_CONTEXT_DB_KEY);
        hasher.update(self.signing_key.as_bytes());
        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash.as_bytes()[..32]);
        key
    }
}

/// Verify a signature against a public key
pub fn verify_signature(
    pubkey_bytes: &[u8; 32],
    message: &[u8],
    signature: &Signature,
) -> Result<(), IdentityError> {
    let verifying_key =
        VerifyingKey::from_bytes(pubkey_bytes).map_err(|_| IdentityError::InvalidKeyBytes)?;
    verifying_key
        .verify(message, signature)
        .map_err(|_| IdentityError::InvalidKeyBytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let id = Identity::generate();
        let user_id = id.user_id();
        assert_eq!(user_id.0.len(), 32);
    }

    #[test]
    fn test_identity_roundtrip() {
        let id = Identity::generate();
        let export = id.to_export();
        let restored = Identity::from_export(&export);
        assert_eq!(id.user_id(), restored.user_id());
    }

    #[test]
    fn test_sign_verify() {
        let id = Identity::generate();
        let message = b"Hello, Liberte!";
        let signature = id.sign(message);

        assert!(verify_signature(&id.public_key_bytes(), message, &signature).is_ok());

        // Wrong message should fail
        assert!(verify_signature(&id.public_key_bytes(), b"wrong", &signature).is_err());
    }

    #[test]
    fn test_db_key_derivation_deterministic() {
        let id = Identity::generate();
        let key1 = id.derive_db_key();
        let key2 = id.derive_db_key();
        assert_eq!(key1, key2);
    }
}
