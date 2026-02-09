use liberte_shared::crypto::SymmetricKey;
use liberte_shared::types::UserId;
use thiserror::Error;
use tracing::{debug, info, warn};

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

/// State of the SFU connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfuState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Client-side SFU (Selective Forwarding Unit) connection.
///
/// Critical security invariant: The SFU NEVER decrypts media.
/// All frames are encrypted client-side via insertable streams before
/// being sent to the SFU. The SFU only routes opaque encrypted packets.
pub struct SfuClient {
    local_user: UserId,
    /// Session encryption key shared between call participants only
    session_key: SymmetricKey,
    /// SFU server address
    server_addr: String,
    /// Current connection state
    state: SfuState,
    /// Room/channel identifier on the SFU
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

    /// Connect to the SFU server
    pub async fn connect(&mut self) -> Result<(), SfuClientError> {
        info!(
            server = %self.server_addr,
            room = %self.room_id,
            "Connecting to SFU"
        );
        self.state = SfuState::Connecting;

        // TODO: Establish WebRTC connection to SFU via webrtc-rs
        // The SFU acts as a peer that receives all media and selectively forwards it
        // Connection would use the relay's multiaddr for signaling

        self.state = SfuState::Connected;
        info!("Connected to SFU");
        Ok(())
    }

    /// Send an encrypted audio frame to the SFU.
    /// The frame is encrypted BEFORE being sent - the SFU cannot read it.
    pub fn encrypt_audio_frame(&self, raw_audio: &[u8]) -> Result<Vec<u8>, SfuClientError> {
        if self.state != SfuState::Connected {
            return Err(SfuClientError::NotConnected);
        }

        let encrypted =
            insertable::encrypt_frame(&self.session_key, insertable::FRAME_TYPE_AUDIO, raw_audio)?;
        Ok(encrypted)
    }

    /// Send an encrypted video frame to the SFU.
    pub fn encrypt_video_frame(&self, raw_video: &[u8]) -> Result<Vec<u8>, SfuClientError> {
        if self.state != SfuState::Connected {
            return Err(SfuClientError::NotConnected);
        }

        let encrypted =
            insertable::encrypt_frame(&self.session_key, insertable::FRAME_TYPE_VIDEO, raw_video)?;
        Ok(encrypted)
    }

    /// Decrypt a received frame from the SFU (forwarded from another participant)
    pub fn decrypt_frame(&self, encrypted_frame: &[u8]) -> Result<(u8, Vec<u8>), SfuClientError> {
        let (frame_type, data) = insertable::decrypt_frame(&self.session_key, encrypted_frame)?;
        Ok((frame_type, data))
    }

    /// Disconnect from the SFU
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
