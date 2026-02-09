use snow::{Builder, HandshakeState, TransportState};

use crate::error::NoiseError;

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub fn build_initiator(local_private_key: &[u8; 32]) -> Result<HandshakeState, NoiseError> {
    Builder::new(
        NOISE_PATTERN
            .parse()
            .map_err(|e| NoiseError::Handshake(format!("{e}")))?,
    )
    .local_private_key(local_private_key)
    .build_initiator()
    .map_err(|e| NoiseError::Handshake(format!("{e}")))
}

pub fn build_responder(local_private_key: &[u8; 32]) -> Result<HandshakeState, NoiseError> {
    Builder::new(
        NOISE_PATTERN
            .parse()
            .map_err(|e| NoiseError::Handshake(format!("{e}")))?,
    )
    .local_private_key(local_private_key)
    .build_responder()
    .map_err(|e| NoiseError::Handshake(format!("{e}")))
}

// Finalize handshake -> transport mode, extract shared key from handshake hash
pub fn into_transport(state: HandshakeState) -> Result<(TransportState, [u8; 32]), NoiseError> {
    let handshake_hash: Vec<u8> = state.get_handshake_hash().to_vec();
    let transport = state
        .into_transport_mode()
        .map_err(|e| NoiseError::Transport(format!("{e}")))?;

    let mut shared_key = [0u8; 32];
    let len = handshake_hash.len().min(32);
    shared_key[..len].copy_from_slice(&handshake_hash[..len]);

    Ok((transport, shared_key))
}

pub fn transport_encrypt(
    transport: &mut TransportState,
    plaintext: &[u8],
) -> Result<Vec<u8>, NoiseError> {
    let mut buf = vec![0u8; plaintext.len() + 64]; // extra space for auth tag
    let len = transport
        .write_message(plaintext, &mut buf)
        .map_err(|e| NoiseError::Transport(format!("{e}")))?;
    buf.truncate(len);
    Ok(buf)
}

pub fn transport_decrypt(
    transport: &mut TransportState,
    ciphertext: &[u8],
) -> Result<Vec<u8>, NoiseError> {
    let mut buf = vec![0u8; ciphertext.len()];
    let len = transport
        .read_message(ciphertext, &mut buf)
        .map_err(|e| NoiseError::Transport(format!("{e}")))?;
    buf.truncate(len);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_handshake() {
        let initiator_key = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let responder_key = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);

        let initiator_bytes: [u8; 32] = initiator_key.to_bytes();
        let responder_bytes: [u8; 32] = responder_key.to_bytes();

        let mut initiator = build_initiator(&initiator_bytes).unwrap();
        let mut responder = build_responder(&responder_bytes).unwrap();

        // Noise_XX 3-message handshake
        // msg1: initiator -> responder (e)
        let mut buf = vec![0u8; 256];
        let len = initiator.write_message(&[], &mut buf).unwrap();
        let msg1 = &buf[..len];

        let mut buf = vec![0u8; 256];
        let _len = responder.read_message(msg1, &mut buf).unwrap();

        // msg2: responder -> initiator (e, ee, s, es)
        let mut buf = vec![0u8; 256];
        let len = responder.write_message(&[], &mut buf).unwrap();
        let msg2 = &buf[..len];

        let mut buf = vec![0u8; 256];
        let _len = initiator.read_message(msg2, &mut buf).unwrap();

        // msg3: initiator -> responder (s, se)
        let mut buf = vec![0u8; 256];
        let len = initiator.write_message(&[], &mut buf).unwrap();
        let msg3 = &buf[..len];

        let mut buf = vec![0u8; 256];
        let _len = responder.read_message(msg3, &mut buf).unwrap();

        // both ready for transport
        let (mut i_transport, i_key) = into_transport(initiator).unwrap();
        let (mut r_transport, r_key) = into_transport(responder).unwrap();

        assert_eq!(i_key, r_key);

        let message = b"Hello from initiator!";
        let encrypted = transport_encrypt(&mut i_transport, message).unwrap();
        let decrypted = transport_decrypt(&mut r_transport, &encrypted).unwrap();
        assert_eq!(decrypted, message);
    }
}
