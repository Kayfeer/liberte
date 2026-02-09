use std::collections::HashMap;

use libp2p::{Multiaddr, PeerId};
use tracing::debug;

use liberte_shared::types::ConnectionMode;

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: Multiaddr,
    pub mode: ConnectionMode,
    pub connected_at: u64,
}

#[derive(Debug, Clone)]
pub struct PeerTracker {
    peers: HashMap<PeerId, ConnectionInfo>,
}

impl PeerTracker {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn on_connected(&mut self, peer_id: PeerId, address: Multiaddr, is_relayed: bool) {
        let mode = if is_relayed {
            ConnectionMode::Relayed
        } else {
            ConnectionMode::Direct
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let info = ConnectionInfo {
            peer_id,
            address: address.clone(),
            mode: mode.clone(),
            connected_at: now,
        };

        debug!(
            peer = %peer_id,
            addr = %address,
            mode = ?mode,
            "Tracking new peer connection"
        );

        self.peers.insert(peer_id, info);
    }

    pub fn on_disconnected(&mut self, peer_id: &PeerId) {
        if self.peers.remove(peer_id).is_some() {
            debug!(peer = %peer_id, "Removed peer from tracker");
        }
    }

    pub fn upgrade_to_direct(&mut self, peer_id: &PeerId, new_address: Multiaddr) {
        if let Some(info) = self.peers.get_mut(peer_id) {
            info.mode = ConnectionMode::Direct;
            info.address = new_address;
            debug!(peer = %peer_id, "Upgraded peer connection to direct");
        }
    }

    pub fn get(&self, peer_id: &PeerId) -> Option<&ConnectionInfo> {
        self.peers.get(peer_id)
    }

    pub fn connection_mode(&self, peer_id: &PeerId) -> ConnectionMode {
        self.peers
            .get(peer_id)
            .map(|info| info.mode.clone())
            .unwrap_or(ConnectionMode::Disconnected)
    }

    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.peers.keys().copied().collect()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn direct_count(&self) -> usize {
        self.peers
            .values()
            .filter(|info| info.mode == ConnectionMode::Direct)
            .count()
    }

    pub fn relayed_count(&self) -> usize {
        self.peers
            .values()
            .filter(|info| info.mode == ConnectionMode::Relayed)
            .count()
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.peers.contains_key(peer_id)
    }

    pub fn all_connections(&self) -> Vec<ConnectionInfo> {
        self.peers.values().cloned().collect()
    }
}

impl Default for PeerTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_peer_id() -> PeerId {
        PeerId::random()
    }

    fn test_addr() -> Multiaddr {
        "/ip4/127.0.0.1/udp/4001/quic-v1".parse().unwrap()
    }

    #[test]
    fn test_connect_disconnect() {
        let mut tracker = PeerTracker::new();
        let peer = test_peer_id();
        let addr = test_addr();

        assert!(!tracker.is_connected(&peer));
        assert_eq!(tracker.peer_count(), 0);

        tracker.on_connected(peer, addr, false);
        assert!(tracker.is_connected(&peer));
        assert_eq!(tracker.peer_count(), 1);
        assert_eq!(tracker.connection_mode(&peer), ConnectionMode::Direct);

        tracker.on_disconnected(&peer);
        assert!(!tracker.is_connected(&peer));
        assert_eq!(tracker.peer_count(), 0);
        assert_eq!(tracker.connection_mode(&peer), ConnectionMode::Disconnected);
    }

    #[test]
    fn test_relayed_connection() {
        let mut tracker = PeerTracker::new();
        let peer = test_peer_id();
        let addr = test_addr();

        tracker.on_connected(peer, addr, true);
        assert_eq!(tracker.connection_mode(&peer), ConnectionMode::Relayed);
        assert_eq!(tracker.relayed_count(), 1);
        assert_eq!(tracker.direct_count(), 0);
    }

    #[test]
    fn test_upgrade_to_direct() {
        let mut tracker = PeerTracker::new();
        let peer = test_peer_id();
        let addr = test_addr();
        let new_addr: Multiaddr = "/ip4/192.168.1.1/udp/4001/quic-v1".parse().unwrap();

        tracker.on_connected(peer, addr, true);
        assert_eq!(tracker.connection_mode(&peer), ConnectionMode::Relayed);

        tracker.upgrade_to_direct(&peer, new_addr);
        assert_eq!(tracker.connection_mode(&peer), ConnectionMode::Direct);
    }

    #[test]
    fn test_connected_peers_list() {
        let mut tracker = PeerTracker::new();
        let p1 = test_peer_id();
        let p2 = test_peer_id();
        let addr = test_addr();

        tracker.on_connected(p1, addr.clone(), false);
        tracker.on_connected(p2, addr, true);

        let peers = tracker.connected_peers();
        assert_eq!(peers.len(), 2);
        assert!(peers.contains(&p1));
        assert!(peers.contains(&p2));
    }
}
