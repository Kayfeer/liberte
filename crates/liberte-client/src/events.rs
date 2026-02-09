//! Tauri event constants and helpers.
//!
//! All cross-boundary event names are defined here so they stay in sync
//! between the Rust backend and the TypeScript frontend.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// Event name constants
// ---------------------------------------------------------------------------

/// A new chat message has been received on a subscribed channel.
pub const EVENT_NEW_MESSAGE: &str = "new-message";

/// A remote peer has connected to this node.
pub const EVENT_PEER_CONNECTED: &str = "peer-connected";

/// A remote peer has disconnected from this node.
pub const EVENT_PEER_DISCONNECTED: &str = "peer-disconnected";

/// The voice/video call state has changed (started, ended, muted, etc.).
pub const EVENT_CALL_STATE_CHANGED: &str = "call-state-changed";

/// The network connection mode has changed (direct / relayed / disconnected).
pub const EVENT_CONNECTION_MODE_CHANGED: &str = "connection-mode-changed";

// ---------------------------------------------------------------------------
// Payload types
// ---------------------------------------------------------------------------

/// Payload emitted with [`EVENT_NEW_MESSAGE`].
#[derive(Debug, Clone, Serialize)]
pub struct NewMessagePayload {
    pub channel_id: String,
    pub sender: String,
    pub timestamp: String,
}

/// Payload emitted with [`EVENT_PEER_CONNECTED`] / [`EVENT_PEER_DISCONNECTED`].
#[derive(Debug, Clone, Serialize)]
pub struct PeerEventPayload {
    pub peer_id: String,
}

/// Payload emitted with [`EVENT_CALL_STATE_CHANGED`].
#[derive(Debug, Clone, Serialize)]
pub struct CallStatePayload {
    pub in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
}

/// Payload emitted with [`EVENT_CONNECTION_MODE_CHANGED`].
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionModePayload {
    pub mode: String,
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Emit a typed event to all windows.
///
/// Wraps [`AppHandle::emit`] with consistent error logging.
pub fn emit_event<S: Serialize + Clone>(
    app: &AppHandle,
    event: &str,
    payload: S,
) {
    if let Err(e) = app.emit(event, payload) {
        tracing::error!(event, error = %e, "Failed to emit Tauri event");
    }
}
