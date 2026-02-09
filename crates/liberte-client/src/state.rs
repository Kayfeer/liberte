//! Application state shared across all Tauri commands.
//!
//! The [`AppState`] struct is wrapped in `Arc<Mutex<>>` and registered with
//! Tauri's managed state system so that every invoke handler can access it.

use liberte_shared::identity::Identity;
use liberte_shared::types::ConnectionMode;
use liberte_store::Database;
use tokio::sync::mpsc;

use liberte_net::SwarmCommand;

/// Central application state.
///
/// Holds the user's identity, database handle, swarm command channel,
/// and runtime flags such as call status and premium subscription.
pub struct AppState {
    /// The user's cryptographic identity (Ed25519 keypair).
    /// `None` until the user creates or loads an identity.
    pub identity: Option<Identity>,

    /// Handle to the local encrypted SQLCipher database.
    /// `None` until the identity is loaded and the DB is opened.
    pub database: Option<Database>,

    /// Sender half of the channel used to dispatch commands to the
    /// libp2p swarm task (dial, publish, subscribe, etc.).
    pub swarm_cmd_tx: Option<mpsc::Sender<SwarmCommand>>,

    /// Current network connection mode (direct / relayed / disconnected).
    pub connection_mode: ConnectionMode,

    /// Whether the user is currently in a voice/video call.
    pub is_in_call: bool,

    /// Whether audio input is muted.
    pub is_muted: bool,

    /// Whether the video camera feed is enabled.
    pub is_video_enabled: bool,

    /// Whether the user has an active premium subscription.
    pub is_premium: bool,

    /// URL of the relay server this client connects to.
    /// Self-hosted users point this to their own instance.
    pub server_url: String,
}

impl AppState {
    /// Create a new, uninitialised application state.
    pub fn new() -> Self {
        Self {
            identity: None,
            database: None,
            swarm_cmd_tx: None,
            connection_mode: ConnectionMode::Disconnected,
            is_in_call: false,
            is_muted: false,
            is_video_enabled: true,
            is_premium: false,
            server_url: String::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
