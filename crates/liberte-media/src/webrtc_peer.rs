use liberte_shared::crypto::SymmetricKey;
use liberte_shared::types::UserId;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum PeerConnectionError {
    #[error("WebRTC error: {0}")]
    WebRtc(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Already connected to peer")]
    AlreadyConnected,

    #[error("Max peers reached")]
    MaxPeersReached,
}

const MAX_MESH_PEERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerState {
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub user_id: UserId,
    pub state: PeerState,
    pub is_muted: bool,
    pub is_video_enabled: bool,
}

/// Manages P2P WebRTC connections in full mesh mode.
pub struct MeshManager {
    local_user: UserId,
    session_key: SymmetricKey,
    peers: HashMap<String, PeerInfo>,
    in_call: bool,
}

impl MeshManager {
    pub fn new(local_user: UserId, session_key: SymmetricKey) -> Self {
        Self {
            local_user,
            session_key,
            peers: HashMap::new(),
            in_call: false,
        }
    }

    pub fn start_call(&mut self) -> Result<(), PeerConnectionError> {
        if self.in_call {
            warn!("Already in a call");
            return Ok(());
        }
        self.in_call = true;
        info!(user = %self.local_user.short(), "Starting mesh call");
        Ok(())
    }

    pub fn add_peer(&mut self, user_id: UserId) -> Result<(), PeerConnectionError> {
        if self.peers.len() >= MAX_MESH_PEERS {
            return Err(PeerConnectionError::MaxPeersReached);
        }

        let key = user_id.to_hex();
        if self.peers.contains_key(&key) {
            return Err(PeerConnectionError::AlreadyConnected);
        }

        debug!(peer = %user_id.short(), "Adding peer to mesh");

        self.peers.insert(
            key,
            PeerInfo {
                user_id,
                state: PeerState::Connecting,
                is_muted: false,
                is_video_enabled: false,
            },
        );

        Ok(())
    }

    pub fn remove_peer(&mut self, user_id: &UserId) {
        let key = user_id.to_hex();
        if self.peers.remove(&key).is_some() {
            debug!(peer = %user_id.short(), "Removed peer from mesh");
        }
    }

    pub fn set_peer_state(&mut self, user_id: &UserId, state: PeerState) {
        let key = user_id.to_hex();
        if let Some(peer) = self.peers.get_mut(&key) {
            peer.state = state;
        }
    }

    pub fn end_call(&mut self) {
        info!("Ending mesh call, disconnecting {} peers", self.peers.len());
        self.peers.clear();
        self.in_call = false;
    }

    pub fn connected_peers(&self) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.state == PeerState::Connected)
            .collect()
    }

    pub fn all_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }

    pub fn is_in_call(&self) -> bool {
        self.in_call
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn session_key(&self) -> &SymmetricKey {
        &self.session_key
    }
}
