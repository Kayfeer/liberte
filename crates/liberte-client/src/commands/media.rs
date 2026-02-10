use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;

use liberte_media::audio::{AudioConfig, AudioEngine};
use liberte_net::SwarmCommand;
use liberte_shared::crypto;
use liberte_shared::protocol::{VoiceEvent, VoiceEventType, VoiceFrame, WireMessage};
use liberte_shared::types::{ChannelId, UserId};

use crate::events::*;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallState {
    pub in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub mode: String,
}

/// Publish a WireMessage on the given channel topic, encrypted with the channel key.
fn publish_wire_message(
    cmd_tx: &tokio::sync::mpsc::Sender<SwarmCommand>,
    channel_id: &str,
    channel_key: &[u8; 32],
    msg: &WireMessage,
) {
    let topic = format!("channel:{channel_id}");
    let Ok(plaintext) = msg.to_bytes() else {
        return;
    };
    let Ok(ciphertext) = crypto::encrypt(channel_key, &plaintext) else {
        return;
    };
    // Use try_send to avoid blocking in sync context
    let _ = cmd_tx.try_send(SwarmCommand::PublishMessage {
        topic,
        data: ciphertext,
    });
}

#[tauri::command]
pub async fn start_call(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
) -> Result<CallState, String> {
    // Extract everything we need from state in one lock
    let (cmd_tx, identity_pubkey, channel_key, app_handle) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

        if guard.is_in_call {
            return Err("Already in a call".into());
        }

        let cmd_tx = guard.swarm_cmd_tx.clone().ok_or("Swarm not started")?;

        let identity = guard.identity.as_ref().ok_or("No identity loaded")?;
        let pubkey = identity.public_key_bytes();

        let db = guard.database.as_ref().ok_or("No database")?;
        let key_hex = db
            .get_channel_key(
                uuid::Uuid::parse_str(&channel_id)
                    .map_err(|e| format!("Invalid channel UUID: {e}"))?,
            )
            .map_err(|e| format!("Channel key not found: {e}"))?;
        let key_bytes = hex::decode(&key_hex).map_err(|e| format!("Invalid key hex: {e}"))?;
        if key_bytes.len() != 32 {
            return Err("Invalid channel key length".into());
        }
        let mut channel_key = [0u8; 32];
        channel_key.copy_from_slice(&key_bytes);

        let app_handle = guard.app_handle.clone();

        (cmd_tx, pubkey, channel_key, app_handle)
    };

    // Create audio engine
    let mut engine = AudioEngine::new(AudioConfig::default());
    let mute_flag = engine.mute_flag();
    let active_flag = engine.active_flag();

    // Channels for audio pipeline
    let (capture_tx, mut capture_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(10);
    let (playback_tx, playback_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(50);

    // Start audio capture & playback
    engine
        .start_capture(capture_tx)
        .map_err(|e| format!("Audio capture error: {e}"))?;
    engine
        .start_playback(playback_rx)
        .map_err(|e| format!("Audio playback error: {e}"))?;

    // Store voice state
    {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard.is_in_call = true;
        guard.is_muted = false;
        guard.call_channel_id = Some(channel_id.clone());
        guard.voice_playback_tx = Some(playback_tx);
        guard.voice_active = Some(active_flag.clone());
        guard.voice_muted = Some(mute_flag.clone());
    }

    // Publish VoiceEvent::Join
    let user_id = UserId(identity_pubkey);
    let ch_uuid = uuid::Uuid::parse_str(&channel_id).unwrap();
    let join_msg = WireMessage::VoiceEvent(VoiceEvent {
        user_id: user_id.clone(),
        channel_id: ChannelId(ch_uuid),
        event_type: VoiceEventType::Join,
        timestamp: chrono::Utc::now(),
    });
    publish_wire_message(&cmd_tx, &channel_id, &channel_key, &join_msg);

    // Spawn voice sender task
    let sender_active = active_flag;
    let sender_muted = mute_flag;
    let sender_channel_id = channel_id.clone();
    let sender_user_id = user_id;
    let sender_cmd_tx = cmd_tx;
    let sender_channel_key = channel_key;

    tokio::spawn(async move {
        let mut sequence: u32 = 0;
        let topic = format!("channel:{sender_channel_id}");
        let ch_uuid = uuid::Uuid::parse_str(&sender_channel_id).unwrap();

        info!("Voice sender task started");

        while sender_active.load(Ordering::Relaxed) {
            // Use timeout so the loop exits promptly when active is set to false
            let frame = match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                capture_rx.recv(),
            )
            .await
            {
                Ok(Some(f)) => f,
                Ok(None) => break,  // capture channel closed
                Err(_) => continue, // timeout — re-check active flag
            };

            if sender_muted.load(Ordering::Relaxed) {
                continue; // Don't send when muted
            }

            // Downsample 48kHz → 16kHz (take every 3rd sample)
            let downsampled: Vec<i16> = frame
                .iter()
                .step_by(3)
                .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
                .collect();

            // Convert i16 to bytes (little-endian)
            let mut audio_bytes = Vec::with_capacity(downsampled.len() * 2);
            for sample in &downsampled {
                audio_bytes.extend_from_slice(&sample.to_le_bytes());
            }

            let voice_frame = WireMessage::VoiceFrame(VoiceFrame {
                sender: sender_user_id.clone(),
                channel_id: ChannelId(ch_uuid),
                sequence,
                audio_data: audio_bytes,
            });

            sequence = sequence.wrapping_add(1);

            if let Ok(plaintext) = voice_frame.to_bytes() {
                if let Ok(ciphertext) = crypto::encrypt(&sender_channel_key, &plaintext) {
                    let _ = sender_cmd_tx
                        .send(SwarmCommand::PublishMessage {
                            topic: topic.clone(),
                            data: ciphertext,
                        })
                        .await;
                }
            }
        }

        info!("Voice sender task ended");
    });

    // Emit event
    if let Some(ref app) = app_handle {
        emit_event(
            app,
            EVENT_CALL_STATE_CHANGED,
            CallStatePayload {
                in_call: true,
                is_muted: false,
                is_video_enabled: true,
            },
        );
    }

    info!(channel = %channel_id, "Voice call started");

    Ok(CallState {
        in_call: true,
        is_muted: false,
        is_video_enabled: true,
        mode: "mesh".to_string(),
    })
}

#[tauri::command]
pub async fn end_call(state: State<'_, Arc<Mutex<AppState>>>) -> Result<CallState, String> {
    let (cmd_tx, channel_id, channel_key, identity_pubkey, app_handle) = {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

        if !guard.is_in_call {
            return Err("Not in a call".into());
        }

        // Stop audio
        if let Some(active) = guard.voice_active.take() {
            active.store(false, Ordering::SeqCst);
        }

        let cmd_tx = guard.swarm_cmd_tx.clone();
        let channel_id = guard.call_channel_id.take();
        let identity_pubkey = guard.identity.as_ref().map(|id| id.public_key_bytes());
        let app_handle = guard.app_handle.clone();

        // Get channel key for leave message
        let channel_key = if let (Some(ref cid), Some(ref db)) = (&channel_id, &guard.database) {
            if let Ok(uuid) = uuid::Uuid::parse_str(cid) {
                db.get_channel_key(uuid)
                    .ok()
                    .and_then(|hex| hex::decode(&hex).ok())
                    .and_then(|b| {
                        if b.len() == 32 {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&b);
                            Some(arr)
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        } else {
            None
        };

        guard.is_in_call = false;
        guard.is_muted = false;
        guard.voice_playback_tx = None;
        guard.voice_muted = None;

        (cmd_tx, channel_id, channel_key, identity_pubkey, app_handle)
    };

    // Publish VoiceEvent::Leave
    if let (Some(cmd_tx), Some(channel_id), Some(channel_key), Some(pubkey)) =
        (cmd_tx, channel_id.as_ref(), channel_key, identity_pubkey)
    {
        let leave_msg = WireMessage::VoiceEvent(VoiceEvent {
            user_id: UserId(pubkey),
            channel_id: ChannelId(uuid::Uuid::parse_str(channel_id).unwrap()),
            event_type: VoiceEventType::Leave,
            timestamp: chrono::Utc::now(),
        });
        publish_wire_message(&cmd_tx, channel_id, &channel_key, &leave_msg);
    }

    // Emit event
    if let Some(ref app) = app_handle {
        emit_event(
            app,
            EVENT_CALL_STATE_CHANGED,
            CallStatePayload {
                in_call: false,
                is_muted: false,
                is_video_enabled: true,
            },
        );
    }

    info!("Voice call ended");

    Ok(CallState {
        in_call: false,
        is_muted: false,
        is_video_enabled: true,
        mode: "mesh".to_string(),
    })
}

#[tauri::command]
pub async fn toggle_mute(state: State<'_, Arc<Mutex<AppState>>>) -> Result<bool, String> {
    let (new_muted, cmd_tx, channel_id, channel_key, identity_pubkey) = {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

        if !guard.is_in_call {
            return Err("Not in a call".into());
        }

        // Toggle mute flag
        let new_muted = !guard.is_muted;
        guard.is_muted = new_muted;

        if let Some(ref mute_flag) = guard.voice_muted {
            mute_flag.store(new_muted, Ordering::SeqCst);
        }

        let cmd_tx = guard.swarm_cmd_tx.clone();
        let channel_id = guard.call_channel_id.clone();
        let identity_pubkey = guard.identity.as_ref().map(|id| id.public_key_bytes());

        let channel_key = if let (Some(ref cid), Some(ref db)) = (&channel_id, &guard.database) {
            if let Ok(uuid) = uuid::Uuid::parse_str(cid) {
                db.get_channel_key(uuid)
                    .ok()
                    .and_then(|hex| hex::decode(&hex).ok())
                    .and_then(|b| {
                        if b.len() == 32 {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&b);
                            Some(arr)
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        } else {
            None
        };

        (new_muted, cmd_tx, channel_id, channel_key, identity_pubkey)
    };

    // Publish mute/unmute event
    if let (Some(cmd_tx), Some(channel_id), Some(channel_key), Some(pubkey)) =
        (cmd_tx, channel_id.as_ref(), channel_key, identity_pubkey)
    {
        let event_type = if new_muted {
            VoiceEventType::Mute
        } else {
            VoiceEventType::Unmute
        };
        let mute_msg = WireMessage::VoiceEvent(VoiceEvent {
            user_id: UserId(pubkey),
            channel_id: ChannelId(uuid::Uuid::parse_str(channel_id).unwrap()),
            event_type,
            timestamp: chrono::Utc::now(),
        });
        publish_wire_message(&cmd_tx, channel_id, &channel_key, &mute_msg);
    }

    info!(muted = new_muted, "Mute toggled");
    Ok(new_muted)
}

#[tauri::command]
pub fn toggle_video(state: State<'_, Arc<Mutex<AppState>>>) -> Result<bool, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if !guard.is_in_call {
        return Err("Not in a call".into());
    }

    guard.is_video_enabled = !guard.is_video_enabled;
    info!(video = guard.is_video_enabled, "Video toggled");

    Ok(guard.is_video_enabled)
}

#[tauri::command]
pub fn set_call_mode(
    state: State<'_, Arc<Mutex<AppState>>>,
    mode: String,
) -> Result<String, String> {
    if mode != "mesh" && mode != "sfu" {
        return Err(format!("Invalid call mode: {mode}. Use 'mesh' or 'sfu'."));
    }

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.call_mode = mode.clone();
    info!(mode = %mode, "Call mode changed");

    Ok(mode)
}

#[tauri::command]
pub fn get_call_state(state: State<'_, Arc<Mutex<AppState>>>) -> Result<CallState, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    Ok(CallState {
        in_call: guard.is_in_call,
        is_muted: guard.is_muted,
        is_video_enabled: guard.is_video_enabled,
        mode: guard.call_mode.clone(),
    })
}
