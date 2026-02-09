//! Voice and video call Tauri commands.
//!
//! These commands control the local call state (start, end, mute, video
//! toggle) and update the shared [`AppState`] so the UI can reflect the
//! current call status.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;

use crate::state::AppState;

/// Response payload describing the current call state.
#[derive(Debug, Clone, Serialize)]
pub struct CallState {
    pub in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
}

/// Start a voice/video call in the given channel.
///
/// In a full implementation this would initialise the WebRTC mesh via
/// `liberte_media::webrtc_peer::MeshManager` and begin signaling.
/// For now it flips the `is_in_call` flag and returns the new state.
#[tauri::command]
pub fn start_call(
    state: State<'_, Arc<Mutex<AppState>>>,
    _channel_id: String,
) -> Result<CallState, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if guard.is_in_call {
        return Err("Already in a call".into());
    }

    guard.is_in_call = true;
    guard.is_muted = false;
    guard.is_video_enabled = true;

    info!("Call started");

    Ok(CallState {
        in_call: true,
        is_muted: false,
        is_video_enabled: true,
    })
}

/// End the current call and clean up media resources.
#[tauri::command]
pub fn end_call(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<CallState, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if !guard.is_in_call {
        return Err("Not in a call".into());
    }

    guard.is_in_call = false;
    guard.is_muted = false;
    guard.is_video_enabled = true;

    info!("Call ended");

    Ok(CallState {
        in_call: false,
        is_muted: false,
        is_video_enabled: true,
    })
}

/// Toggle audio mute state. Returns the new call state.
#[tauri::command]
pub fn toggle_mute(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<CallState, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if !guard.is_in_call {
        return Err("Not in a call".into());
    }

    guard.is_muted = !guard.is_muted;

    info!(muted = guard.is_muted, "Mute toggled");

    Ok(CallState {
        in_call: true,
        is_muted: guard.is_muted,
        is_video_enabled: guard.is_video_enabled,
    })
}

/// Toggle video camera on/off. Returns the new call state.
#[tauri::command]
pub fn toggle_video(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<CallState, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if !guard.is_in_call {
        return Err("Not in a call".into());
    }

    guard.is_video_enabled = !guard.is_video_enabled;

    info!(video = guard.is_video_enabled, "Video toggled");

    Ok(CallState {
        in_call: true,
        is_muted: guard.is_muted,
        is_video_enabled: guard.is_video_enabled,
    })
}
