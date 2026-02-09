use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum VideoError {
    #[error("No video device available")]
    NoDevice,

    #[error("Video capture error: {0}")]
    CaptureError(String),

    #[error("Video encode error: {0}")]
    EncodeError(String),

    #[error("Video decode error: {0}")]
    DecodeError(String),
}

/// Video configuration
#[derive(Debug, Clone)]
pub struct VideoConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 30,
            bitrate_kbps: 2500,
        }
    }
}

/// Video frame data
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub timestamp_ms: u64,
}

/// Manages video capture and encoding pipeline
pub struct VideoEngine {
    config: VideoConfig,
    is_capturing: bool,
    is_enabled: bool,
}

impl VideoEngine {
    pub fn new(config: VideoConfig) -> Self {
        Self {
            config,
            is_capturing: false,
            is_enabled: true,
        }
    }

    /// Start video capture.
    /// Frames are encoded and sent via the provided channel.
    /// Note: Actual camera capture will use webrtc-rs built-in capture
    /// or platform-specific APIs. This is the processing pipeline.
    pub fn start(&mut self) -> Result<(), VideoError> {
        info!(
            width = self.config.width,
            height = self.config.height,
            fps = self.config.fps,
            "Starting video engine"
        );
        self.is_capturing = true;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_capturing = false;
        debug!("Video engine stopped");
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
        debug!(enabled, "Video enabled state changed");
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }

    pub fn config(&self) -> &VideoConfig {
        &self.config
    }
}
