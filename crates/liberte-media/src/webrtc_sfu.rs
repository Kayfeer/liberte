use liberte_shared::crypto::SymmetricKey;
use liberte_shared::types::UserId;
use thiserror::Error;
use tracing::info;

use crate::insertable;

#[derive(Error, Debug)]
pub enum SfuClientError {
    #[error("SFU connection error: {0}")]
    ConnectionError(String),

    #[error("Not connected to SFU")]
    NotConnected,

    #[error("Frame encryption error: {0}")]
    FrameError(#[from] insertable::FrameError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfuState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Client-side SFU connection.
/// The SFU never decrypts media -- all frames are E2EE via insertable streams.
pub struct SfuClient {
    local_user: UserId,
    session_key: SymmetricKey,
    server_addr: String,
    state: SfuState,
    room_id: String,
}

impl SfuClient {
    pub fn new(
        local_user: UserId,
        session_key: SymmetricKey,
        server_addr: String,
        room_id: String,
    ) -> Self {
        Self {
            local_user,
            session_key,
            server_addr,
            state: SfuState::Disconnected,
            room_id,
        }
    }

    pub async fn connect(&mut self) -> Result<(), SfuClientError> {
        info!(
            server = %self.server_addr,
            room = %self.room_id,
            "Connecting to SFU"
        );
        self.state = SfuState::Connecting;

        // TODO: Establish WebRTC connection to SFU via webrtc-rs
        // The SFU acts as a peer that receives all media and selectively forwards it

        self.state = SfuState::Connected;
        info!("Connected to SFU");
        Ok(())
    }

    /// Encrypt an audio frame before sending to the SFU (SFU can't read it).
    pub fn encrypt_audio_frame(&self, raw_audio: &[u8]) -> Result<Vec<u8>, SfuClientError> {
        if self.state != SfuState::Connected {
            return Err(SfuClientError::NotConnected);
        }

        let encrypted =
            insertable::encrypt_frame(&self.session_key, insertable::FRAME_TYPE_AUDIO, raw_audio)?;
        Ok(encrypted)
    }

    pub fn encrypt_video_frame(&self, raw_video: &[u8]) -> Result<Vec<u8>, SfuClientError> {
        if self.state != SfuState::Connected {
            return Err(SfuClientError::NotConnected);
        }

        let encrypted =
            insertable::encrypt_frame(&self.session_key, insertable::FRAME_TYPE_VIDEO, raw_video)?;
        Ok(encrypted)
    }

    pub fn decrypt_frame(&self, encrypted_frame: &[u8]) -> Result<(u8, Vec<u8>), SfuClientError> {
        let (frame_type, data) = insertable::decrypt_frame(&self.session_key, encrypted_frame)?;
        Ok((frame_type, data))
    }

    pub async fn disconnect(&mut self) {
        info!("Disconnecting from SFU");
        self.state = SfuState::Disconnected;
    }

    pub fn state(&self) -> &SfuState {
        &self.state
    }

    pub fn is_connected(&self) -> bool {
        self.state == SfuState::Connected
    }

    pub fn room_id(&self) -> &str {
        &self.room_id
    }
}
