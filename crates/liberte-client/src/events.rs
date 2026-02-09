use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_NEW_MESSAGE: &str = "new-message";
pub const EVENT_PEER_CONNECTED: &str = "peer-connected";
pub const EVENT_PEER_DISCONNECTED: &str = "peer-disconnected";
pub const EVENT_CALL_STATE_CHANGED: &str = "call-state-changed";
pub const EVENT_CONNECTION_MODE_CHANGED: &str = "connection-mode-changed";

#[derive(Debug, Clone, Serialize)]
pub struct NewMessagePayload {
    pub channel_id: String,
    pub sender: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerEventPayload {
    pub peer_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallStatePayload {
    pub in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionModePayload {
    pub mode: String,
}

pub fn emit_event<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        tracing::error!(event, error = %e, "Failed to emit event");
    }
}
