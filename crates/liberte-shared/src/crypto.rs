use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;

use crate::constants::{KDF_CONTEXT_CHANNEL_KEY, NONCE_SIZE};
use crate::error::CryptoError;

pub type SymmetricKey = [u8; 32];

pub fn generate_symmetric_key() -> SymmetricKey {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

pub fn generate_nonce() -> [u8; NONCE_SIZE] {
    let mut nonce = [0u8; NONCE_SIZE];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    nonce
}

// Returns nonce || ciphertext (24 bytes nonce prepended)
pub fn encrypt(key: &SymmetricKey, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce_bytes = generate_nonce();
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt(key: &SymmetricKey, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < NONCE_SIZE {
        return Err(CryptoError::DecryptionFailed);
    }

    let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)
}

// BLAKE3 KDF with domain separation
pub fn derive_channel_key(shared_secret: &[u8], channel_id: &[u8]) -> SymmetricKey {
    let mut hasher = blake3::Hasher::new_derive_key(KDF_CONTEXT_CHANNEL_KEY);
    hasher.update(shared_secret);
    hasher.update(channel_id);
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash.as_bytes()[..32]);
    key
}

pub fn derive_key_from_passphrase(passphrase: &[u8], context: &str) -> SymmetricKey {
    let mut hasher = blake3::Hasher::new_derive_key(context);
    hasher.update(passphrase);
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash.as_bytes()[..32]);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_symmetric_key();
        let plaintext = b"Liberte, egalite, fraternite!";

        let encrypted = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = generate_symmetric_key();
        let key2 = generate_symmetric_key();
        let plaintext = b"Secret message";

        let encrypted = encrypt(&key1, plaintext).unwrap();
        assert!(decrypt(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = generate_symmetric_key();
        let plaintext = b"Important data";

        let mut encrypted = encrypt(&key, plaintext).unwrap();
        let len = encrypted.len();
        encrypted[len - 1] ^= 0xFF;

        assert!(decrypt(&key, &encrypted).is_err());
    }

    #[test]
    fn test_empty_data_fails() {
        let key = generate_symmetric_key();
        assert!(decrypt(&key, &[]).is_err());
    }

    #[test]
    fn test_channel_key_derivation_deterministic() {
        let secret = b"shared-secret-between-peers";
        let channel_id = b"channel-123";

        let key1 = derive_channel_key(secret, channel_id);
        let key2 = derive_channel_key(secret, channel_id);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_different_channels_different_keys() {
        let secret = b"shared-secret";
        let key1 = derive_channel_key(secret, b"channel-1");
        let key2 = derive_channel_key(secret, b"channel-2");

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_nonce_prepended() {
        let key = generate_symmetric_key();
        let encrypted = encrypt(&key, b"test").unwrap();
        // nonce (24) + ciphertext (4 + 16 tag)
        assert!(encrypted.len() >= NONCE_SIZE + 4 + 16);
    }
}
