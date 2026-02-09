use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No input device available")]
    NoInputDevice,

    #[error("No output device available")]
    NoOutputDevice,

    #[error("Audio device error: {0}")]
    DeviceError(String),

    #[error("Audio stream error: {0}")]
    StreamError(String),
}

/// Audio configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_size_ms: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1, // Mono for voice
            frame_size_ms: 20, // 20ms frames (Opus standard)
        }
    }
}

impl AudioConfig {
    /// Number of samples per frame
    pub fn frame_size_samples(&self) -> usize {
        (self.sample_rate as usize * self.frame_size_ms as usize) / 1000
    }
}

/// Manages audio capture and playback
pub struct AudioEngine {
    config: AudioConfig,
    is_capturing: bool,
    is_muted: bool,
}

impl AudioEngine {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            is_capturing: false,
            is_muted: false,
        }
    }

    /// Start capturing audio from the default input device.
    /// Captured frames are sent via the provided channel.
    pub fn start_capture(
        &mut self,
        frame_tx: tokio::sync::mpsc::Sender<Vec<f32>>,
    ) -> Result<(), AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;

        info!(device = ?device.name(), "Using input device");

        let config = cpal::StreamConfig {
            channels: self.config.channels,
            sample_rate: cpal::SampleRate(self.config.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let frame_size = self.config.frame_size_samples();
        let mut buffer = Vec::with_capacity(frame_size);

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    buffer.extend_from_slice(data);
                    while buffer.len() >= frame_size {
                        let frame: Vec<f32> = buffer.drain(..frame_size).collect();
                        if frame_tx.try_send(frame).is_err() {
                            warn!("Audio frame channel full, dropping frame");
                        }
                    }
                },
                move |err| {
                    error!("Audio input error: {err}");
                },
                None,
            )
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        // Keep stream alive by leaking it (will be cleaned up on drop of AudioEngine)
        std::mem::forget(stream);

        self.is_capturing = true;
        debug!("Audio capture started");
        Ok(())
    }

    /// Start playing audio frames from the provided channel.
    pub fn start_playback(
        &mut self,
        mut frame_rx: tokio::sync::mpsc::Receiver<Vec<f32>>,
    ) -> Result<(), AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;

        info!(device = ?device.name(), "Using output device");

        let config = cpal::StreamConfig {
            channels: self.config.channels,
            sample_rate: cpal::SampleRate(self.config.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let (playback_tx, playback_rx) = std::sync::mpsc::channel::<Vec<f32>>();

        // Spawn a task to bridge tokio channel to std channel
        tokio::spawn(async move {
            while let Some(frame) = frame_rx.recv().await {
                if playback_tx.send(frame).is_err() {
                    break;
                }
            }
        });

        let mut play_buffer: Vec<f32> = Vec::new();

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    // Drain available frames into play buffer
                    while let Ok(frame) = playback_rx.try_recv() {
                        play_buffer.extend_from_slice(&frame);
                    }

                    for sample in data.iter_mut() {
                        *sample = if play_buffer.is_empty() {
                            0.0 // Silence when no data
                        } else {
                            play_buffer.remove(0)
                        };
                    }
                },
                move |err| {
                    error!("Audio output error: {err}");
                },
                None,
            )
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        std::mem::forget(stream);
        debug!("Audio playback started");
        Ok(())
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted = muted;
        debug!(muted, "Audio mute state changed");
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }
}
