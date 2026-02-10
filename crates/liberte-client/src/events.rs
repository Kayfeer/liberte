use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_NEW_MESSAGE: &str = "new-message";
pub const EVENT_PEER_CONNECTED: &str = "peer-connected";
pub const EVENT_PEER_DISCONNECTED: &str = "peer-disconnected";
pub const EVENT_CALL_STATE_CHANGED: &str = "call-state-changed";
pub const EVENT_CONNECTION_MODE_CHANGED: &str = "connection-mode-changed";
pub const EVENT_TYPING_INDICATOR: &str = "typing-indicator";
pub const EVENT_STATUS_CHANGED: &str = "status-changed";
pub const EVENT_MESSAGE_REACTION: &str = "message-reaction";
pub const EVENT_VOICE_PEER_JOINED: &str = "voice-peer-joined";
pub const EVENT_VOICE_PEER_LEFT: &str = "voice-peer-left";
pub const EVENT_VOICE_PEER_MUTED: &str = "voice-peer-muted";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMessagePayload {
    pub channel_id: String,
    pub sender: String,
    pub message_id: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingPayload {
    pub channel_id: String,
    pub user_id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChangedPayload {
    pub user_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionPayload {
    pub channel_id: String,
    pub message_id: String,
    pub user_id: String,
    pub emoji: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePeerPayload {
    pub channel_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePeerMutedPayload {
    pub channel_id: String,
    pub user_id: String,
    pub muted: bool,
}

pub fn emit_event<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        tracing::error!(event, error = %e, "Failed to emit event");
    }
}
