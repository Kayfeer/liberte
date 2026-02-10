use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use liberte_shared::identity::Identity;
use liberte_shared::types::ConnectionMode;
use liberte_store::Database;
use tauri::AppHandle;
use tokio::sync::mpsc;

use liberte_net::SwarmCommand;

pub struct AppState {
    pub identity: Option<Identity>,
    pub database: Option<Database>,
    pub swarm_cmd_tx: Option<mpsc::Sender<SwarmCommand>>,
    pub app_handle: Option<AppHandle>,
    pub connection_mode: ConnectionMode,
    // Voice call state
    pub is_in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub is_premium: bool,
    pub server_url: String,
    pub call_mode: String,
    /// Channel UUID string of the active voice call
    pub call_channel_id: Option<String>,
    /// Send incoming voice frames here for playback mixing
    pub voice_playback_tx: Option<mpsc::Sender<Vec<f32>>>,
    /// Set to false to stop the voice sender task
    pub voice_active: Option<Arc<AtomicBool>>,
    /// Mute flag shared with AudioEngine
    pub voice_muted: Option<Arc<AtomicBool>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            identity: None,
            database: None,
            swarm_cmd_tx: None,
            app_handle: None,
            connection_mode: ConnectionMode::Disconnected,
            is_in_call: false,
            is_muted: false,
            is_video_enabled: true,
            is_premium: false,
            server_url: String::new(),
            call_mode: "mesh".to_string(),
            call_channel_id: None,
            voice_playback_tx: None,
            voice_active: None,
            voice_muted: None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
