use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
            channels: 1,
            frame_size_ms: 20,
        }
    }
}

impl AudioConfig {
    pub fn frame_size_samples(&self) -> usize {
        (self.sample_rate as usize * self.frame_size_ms as usize * self.channels as usize) / 1000
    }
}

pub struct AudioEngine {
    config: AudioConfig,
    is_capturing: bool,
    is_muted: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,
    playback_bridge: Option<tokio::task::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            is_capturing: false,
            is_muted: Arc::new(AtomicBool::new(false)),
            active: Arc::new(AtomicBool::new(false)),
            input_stream: None,
            output_stream: None,
            playback_bridge: None,
        }
    }

    pub fn mute_flag(&self) -> Arc<AtomicBool> {
        self.is_muted.clone()
    }

    pub fn active_flag(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }

    pub fn start_capture(
        &mut self,
        frame_tx: tokio::sync::mpsc::Sender<Vec<f32>>,
    ) -> Result<(), AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        if self.input_stream.is_some() {
            return Err(AudioError::StreamError("Capture already started".into()));
        }
        if self.config.frame_size_samples() == 0 {
            return Err(AudioError::StreamError(
                "Invalid audio frame configuration".into(),
            ));
        }

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
        let muted = self.is_muted.clone();
        let active = self.active.clone();

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    if !active.load(Ordering::Relaxed) {
                        return;
                    }
                    if muted.load(Ordering::Relaxed) {
                        // Send silence when muted so playback stays in sync
                        buffer.extend(std::iter::repeat_n(0.0f32, data.len()));
                    } else {
                        buffer.extend_from_slice(data);
                    }
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

        self.active.store(true, Ordering::SeqCst);
        self.input_stream = Some(stream);

        self.is_capturing = true;
        debug!("Audio capture started");
        Ok(())
    }

    pub fn start_playback(
        &mut self,
        mut frame_rx: tokio::sync::mpsc::Receiver<Vec<f32>>,
    ) -> Result<(), AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        if self.output_stream.is_some() {
            return Err(AudioError::StreamError("Playback already started".into()));
        }

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

        let (playback_tx, playback_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(4);
        let active = self.active.clone();

        // Bridge tokio channel to std channel for the audio callback
        let mut play_buffer: std::collections::VecDeque<f32> = std::collections::VecDeque::new();
        let max_samples = self.config.sample_rate as usize * self.config.channels as usize / 4;

        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    if !active.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    // Drain available frames into play buffer
                    while let Ok(frame) = playback_rx.try_recv() {
                        append_bounded(&mut play_buffer, &frame, max_samples);
                    }

                    for sample in data.iter_mut() {
                        *sample = play_buffer.pop_front().unwrap_or(0.0);
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

        self.active.store(true, Ordering::SeqCst);
        self.output_stream = Some(stream);
        let active_bridge = self.active.clone();
        self.playback_bridge = Some(tokio::spawn(async move {
            while active_bridge.load(Ordering::Relaxed) {
                let Some(frame) = frame_rx.recv().await else {
                    break;
                };
                // Never block a runtime worker or accumulate unbounded audio.
                match playback_tx.try_send(frame) {
                    Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => {}
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
        }));
        debug!("Audio playback started");
        Ok(())
    }

    pub fn stop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        self.input_stream.take();
        self.output_stream.take();
        if let Some(bridge) = self.playback_bridge.take() {
            bridge.abort();
        }
        self.is_capturing = false;
        // Reset mute state
        self.is_muted.store(false, Ordering::SeqCst);
        debug!("Audio engine stopped");
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted.store(muted, Ordering::SeqCst);
        debug!(muted, "Audio mute state changed");
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn append_bounded(buffer: &mut std::collections::VecDeque<f32>, frame: &[f32], max_samples: usize) {
    let frame = &frame[frame.len().saturating_sub(max_samples)..];
    let excess = buffer
        .len()
        .saturating_add(frame.len())
        .saturating_sub(max_samples);
    buffer.drain(..excess.min(buffer.len()));
    buffer.extend(frame.iter().copied());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_drops_old_audio_instead_of_growing_latency() {
        let mut buffer = std::collections::VecDeque::from(vec![1.0, 2.0, 3.0]);
        append_bounded(&mut buffer, &[4.0, 5.0, 6.0], 4);
        assert_eq!(
            buffer.into_iter().collect::<Vec<_>>(),
            vec![3.0, 4.0, 5.0, 6.0]
        );
        let mut buffer = std::collections::VecDeque::new();
        append_bounded(&mut buffer, &[1.0, 2.0, 3.0], 2);
        assert_eq!(buffer.into_iter().collect::<Vec<_>>(), vec![2.0, 3.0]);
    }
    #[test]
    fn stereo_frames_include_both_channels() {
        assert_eq!(
            AudioConfig {
                channels: 2,
                ..AudioConfig::default()
            }
            .frame_size_samples(),
            1920
        );
    }
}
