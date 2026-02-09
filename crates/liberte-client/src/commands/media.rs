use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallState {
    pub in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub mode: String,
}

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

    info!(mode = %guard.call_mode, "Call started");

    Ok(CallState {
        in_call: true,
        is_muted: false,
        is_video_enabled: true,
        mode: guard.call_mode.clone(),
    })
}

#[tauri::command]
pub fn end_call(state: State<'_, Arc<Mutex<AppState>>>) -> Result<CallState, String> {
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
        mode: guard.call_mode.clone(),
    })
}

#[tauri::command]
pub fn toggle_mute(state: State<'_, Arc<Mutex<AppState>>>) -> Result<bool, String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    if !guard.is_in_call {
        return Err("Not in a call".into());
    }

    guard.is_muted = !guard.is_muted;
    info!(muted = guard.is_muted, "Mute toggled");

    Ok(guard.is_muted)
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

/// Set call mode: "mesh" for peer-to-peer, "sfu" for server-relayed group calls.
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

/// Get current call state.
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
