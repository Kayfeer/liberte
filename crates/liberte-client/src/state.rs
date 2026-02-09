use liberte_shared::identity::Identity;
use liberte_shared::types::ConnectionMode;
use liberte_store::Database;
use tokio::sync::mpsc;

use liberte_net::SwarmCommand;

pub struct AppState {
    pub identity: Option<Identity>,
    pub database: Option<Database>,
    pub swarm_cmd_tx: Option<mpsc::Sender<SwarmCommand>>,
    pub connection_mode: ConnectionMode,
    pub is_in_call: bool,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub is_premium: bool,
    pub server_url: String,
}

impl AppState {
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
