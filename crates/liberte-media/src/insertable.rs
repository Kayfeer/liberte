use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use liberte_shared::crypto::SymmetricKey;
use rand::RngCore;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrameError {
    #[error("Frame encryption failed")]
    EncryptionFailed,

    #[error("Frame decryption failed")]
    DecryptionFailed,

    #[error("Invalid frame format")]
    InvalidFormat,
}

/// Frame type byte constants
pub const FRAME_TYPE_AUDIO: u8 = 0x01;
pub const FRAME_TYPE_VIDEO: u8 = 0x02;

/// Minimum frame size: 1 byte type + 24 bytes nonce + 16 bytes poly1305 tag
const MIN_FRAME_SIZE: usize = 1 + 24 + 16;

/// Encrypts a media frame (audio or video) before it enters the WebRTC transport.
/// The SFU will forward these encrypted frames without being able to decrypt them.
///
/// Frame format: `[1 byte frame_type][24 bytes nonce][N bytes encrypted_payload]`
pub fn encrypt_frame(
    key: &SymmetricKey,
    frame_type: u8,
    payload: &[u8],
) -> Result<Vec<u8>, FrameError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|_| FrameError::EncryptionFailed)?;

    let mut frame = Vec::with_capacity(1 + 24 + ciphertext.len());
    frame.push(frame_type);
    frame.extend_from_slice(&nonce_bytes);
    frame.extend_from_slice(&ciphertext);
    Ok(frame)
}

/// Decrypts a media frame received from the SFU or a peer.
/// Returns `(frame_type, decrypted_payload)`.
pub fn decrypt_frame(key: &SymmetricKey, encrypted_frame: &[u8]) -> Result<(u8, Vec<u8>), FrameError> {
    if encrypted_frame.len() < MIN_FRAME_SIZE {
        return Err(FrameError::InvalidFormat);
    }

    let frame_type = encrypted_frame[0];
    let nonce = XNonce::from_slice(&encrypted_frame[1..25]);
    let ciphertext = &encrypted_frame[25..];

    let cipher = XChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| FrameError::DecryptionFailed)?;

    Ok((frame_type, plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use liberte_shared::crypto::generate_symmetric_key;

    #[test]
    fn test_frame_encrypt_decrypt_audio() {
        let key = generate_symmetric_key();
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];

        let encrypted = encrypt_frame(&key, FRAME_TYPE_AUDIO, &payload).unwrap();
        let (frame_type, decrypted) = decrypt_frame(&key, &encrypted).unwrap();

        assert_eq!(frame_type, FRAME_TYPE_AUDIO);
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_frame_encrypt_decrypt_video() {
        let key = generate_symmetric_key();
        let payload = vec![0; 1024]; // Simulated video frame

        let encrypted = encrypt_frame(&key, FRAME_TYPE_VIDEO, &payload).unwrap();
        let (frame_type, decrypted) = decrypt_frame(&key, &encrypted).unwrap();

        assert_eq!(frame_type, FRAME_TYPE_VIDEO);
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = generate_symmetric_key();
        let key2 = generate_symmetric_key();
        let payload = vec![1, 2, 3];

        let encrypted = encrypt_frame(&key1, FRAME_TYPE_AUDIO, &payload).unwrap();
        assert!(decrypt_frame(&key2, &encrypted).is_err());
    }

    #[test]
    fn test_invalid_frame_too_short() {
        let key = generate_symmetric_key();
        assert!(decrypt_frame(&key, &[0x01, 0x02]).is_err());
    }

    #[test]
    fn test_tampered_frame_fails() {
        let key = generate_symmetric_key();
        let mut encrypted = encrypt_frame(&key, FRAME_TYPE_AUDIO, b"test").unwrap();
        let len = encrypted.len();
        encrypted[len - 1] ^= 0xFF;
        assert!(decrypt_frame(&key, &encrypted).is_err());
    }
}
