//! Main swarm orchestration with tokio mpsc command/notification pattern.
//!
//! The swarm event loop runs in a dedicated tokio task. External code
//! communicates with it through typed command and notification channels,
//! keeping the networking layer fully asynchronous and decoupled.

use std::path::PathBuf;

use futures::StreamExt;
use libp2p::{
    gossipsub, identify, kad,
    multiaddr::Protocol,
    relay,
    swarm::SwarmEvent,
    Multiaddr, PeerId,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::behaviour::LiberteEvent;
use crate::discovery::load_bootstrap_peers;
use crate::peers::PeerTracker;
use crate::transport::build_swarm;

use liberte_shared::constants::DEFAULT_QUIC_PORT;

// ---------------------------------------------------------------------------
// Command / notification types
// ---------------------------------------------------------------------------

/// Commands sent *into* the swarm task.
#[derive(Debug)]
pub enum SwarmCommand {
    /// Dial a remote peer at the given multiaddr.
    Dial(Multiaddr),
    /// Publish a message on a GossipSub topic.
    PublishMessage {
        topic: String,
        data: Vec<u8>,
    },
    /// Subscribe to a GossipSub topic.
    SubscribeTopic(String),
    /// Request a snapshot of currently connected peers.
    GetPeers(tokio::sync::oneshot::Sender<Vec<PeerId>>),
    /// Gracefully shut down the swarm.
    Shutdown,
}

/// Notifications sent *from* the swarm task to the application.
#[derive(Debug, Clone)]
pub enum SwarmNotification {
    /// A new peer connected.
    PeerConnected {
        peer_id: PeerId,
        address: Multiaddr,
    },
    /// A peer disconnected.
    PeerDisconnected {
        peer_id: PeerId,
    },
    /// A GossipSub message was received.
    MessageReceived {
        source: Option<PeerId>,
        topic: String,
        data: Vec<u8>,
    },
    /// A relay reservation was accepted.
    RelayReservation {
        relay_peer: PeerId,
        relay_addr: Multiaddr,
    },
}

/// Configuration for spawning the swarm.
pub struct SwarmConfig {
    /// Path to the bootstrap peers configuration file.
    pub bootstrap_peers_path: Option<PathBuf>,
    /// Port to listen on (defaults to `DEFAULT_QUIC_PORT`).
    pub listen_port: u16,
    /// Additional multiaddrs to dial on startup.
    pub extra_dials: Vec<Multiaddr>,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers_path: None,
            listen_port: DEFAULT_QUIC_PORT,
            extra_dials: Vec::new(),
        }
    }
}

/// Spawn the libp2p swarm in a background tokio task.
///
/// Returns channels for sending commands and receiving notifications,
/// plus the local `PeerId`.
///
/// # Arguments
///
/// * `keypair` - The node's identity keypair
/// * `config` - Swarm configuration (bootstrap peers, listen port, etc.)
///
/// # Returns
///
/// `(command_tx, notification_rx, local_peer_id)`
pub async fn spawn_swarm(
    keypair: libp2p::identity::Keypair,
    config: SwarmConfig,
) -> anyhow::Result<(
    mpsc::Sender<SwarmCommand>,
    mpsc::Receiver<SwarmNotification>,
    PeerId,
)> {
    // Build the swarm via SwarmBuilder (QUIC + Relay + Behaviour)
    let mut swarm = build_swarm(keypair)?;
    let local_peer_id = *swarm.local_peer_id();

    // Listen on QUIC (IPv4 and IPv6)
    let listen_addr_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.listen_port)
        .parse()
        .expect("valid multiaddr");
    let listen_addr_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", config.listen_port)
        .parse()
        .expect("valid multiaddr");

    swarm.listen_on(listen_addr_v4)?;
    swarm.listen_on(listen_addr_v6)?;

    info!(peer_id = %local_peer_id, port = config.listen_port, "Swarm listening");

    // Load and dial bootstrap peers
    if let Some(ref path) = config.bootstrap_peers_path {
        let bootstrap_addrs = load_bootstrap_peers(path);
        for addr in &bootstrap_addrs {
            if let Err(e) = swarm.dial(addr.clone()) {
                warn!(addr = %addr, error = %e, "Failed to dial bootstrap peer");
            } else {
                // Also add to Kademlia routing table
                if let Some(peer_id) = extract_peer_id(addr) {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
                }
                debug!(addr = %addr, "Dialing bootstrap peer");
            }
        }

        // Kick off Kademlia bootstrap
        if !bootstrap_addrs.is_empty() {
            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                warn!(error = %e, "Kademlia bootstrap failed to start");
            }
        }
    }

    // Dial any extra addresses
    for addr in &config.extra_dials {
        if let Err(e) = swarm.dial(addr.clone()) {
            warn!(addr = %addr, error = %e, "Failed to dial extra address");
        }
    }

    // Create channels
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(256);
    let (notif_tx, notif_rx) = mpsc::channel::<SwarmNotification>(256);

    // Spawn the event loop
    tokio::spawn(async move {
        let mut peer_tracker = PeerTracker::new();

        loop {
            tokio::select! {
                // --- Incoming commands ---
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(SwarmCommand::Dial(addr)) => {
                            if let Err(e) = swarm.dial(addr.clone()) {
                                error!(addr = %addr, error = %e, "Dial failed");
                            }
                        }
                        Some(SwarmCommand::PublishMessage { topic, data }) => {
                            let gossipsub_topic = gossipsub::IdentTopic::new(&topic);
                            if let Err(e) = swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(gossipsub_topic, data)
                            {
                                error!(topic = %topic, error = %e, "Publish failed");
                            }
                        }
                        Some(SwarmCommand::SubscribeTopic(topic)) => {
                            let gossipsub_topic = gossipsub::IdentTopic::new(&topic);
                            if let Err(e) = swarm
                                .behaviour_mut()
                                .gossipsub
                                .subscribe(&gossipsub_topic)
                            {
                                error!(topic = %topic, error = %e, "Subscribe failed");
                            }
                        }
                        Some(SwarmCommand::GetPeers(reply)) => {
                            let peers = peer_tracker.connected_peers();
                            let _ = reply.send(peers);
                        }
                        Some(SwarmCommand::Shutdown) => {
                            info!("Swarm shutdown requested");
                            break;
                        }
                        None => {
                            // All senders dropped
                            info!("Command channel closed, shutting down swarm");
                            break;
                        }
                    }
                }

                // --- Swarm events ---
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(LiberteEvent::Gossipsub(
                            gossipsub::Event::Message {
                                propagation_source: _,
                                message_id: _,
                                message,
                            },
                        )) => {
                            let topic = message.topic.to_string();
                            debug!(
                                topic = %topic,
                                source = ?message.source,
                                len = message.data.len(),
                                "GossipSub message received"
                            );
                            let _ = notif_tx
                                .send(SwarmNotification::MessageReceived {
                                    source: message.source,
                                    topic,
                                    data: message.data,
                                })
                                .await;
                        }

                        SwarmEvent::Behaviour(LiberteEvent::Kademlia(
                            kad::Event::OutboundQueryProgressed { result, .. },
                        )) => {
                            debug!(result = ?result, "Kademlia query progressed");
                        }

                        SwarmEvent::Behaviour(LiberteEvent::Identify(
                            identify::Event::Received { peer_id, info, .. },
                        )) => {
                            debug!(
                                peer = %peer_id,
                                protocol = ?info.protocol_version,
                                "Identify: received info from peer"
                            );
                            // Add observed addresses to Kademlia
                            for addr in &info.listen_addrs {
                                swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, addr.clone());
                            }
                        }

                        SwarmEvent::Behaviour(LiberteEvent::RelayClient(
                            relay::client::Event::ReservationReqAccepted {
                                relay_peer_id,
                                ..
                            },
                        )) => {
                            info!(
                                relay = %relay_peer_id,
                                "Relay reservation accepted"
                            );
                            // Try to find the relay address from external addresses
                            let relay_addr = swarm
                                .external_addresses()
                                .next()
                                .cloned()
                                .unwrap_or_else(Multiaddr::empty);
                            let _ = notif_tx
                                .send(SwarmNotification::RelayReservation {
                                    relay_peer: relay_peer_id,
                                    relay_addr,
                                })
                                .await;
                        }

                        SwarmEvent::Behaviour(LiberteEvent::Dcutr(event)) => {
                            debug!(event = ?event, "DCUtR event");
                        }

                        SwarmEvent::ConnectionEstablished {
                            peer_id, endpoint, ..
                        } => {
                            let addr = endpoint.get_remote_address().clone();
                            let is_relayed = addr.iter().any(|p| matches!(p, Protocol::P2pCircuit));
                            peer_tracker.on_connected(peer_id, addr.clone(), is_relayed);

                            info!(
                                peer = %peer_id,
                                addr = %addr,
                                relayed = is_relayed,
                                "Peer connected"
                            );
                            let _ = notif_tx
                                .send(SwarmNotification::PeerConnected {
                                    peer_id,
                                    address: addr,
                                })
                                .await;
                        }

                        SwarmEvent::ConnectionClosed {
                            peer_id,
                            num_established,
                            ..
                        } => {
                            if num_established == 0 {
                                peer_tracker.on_disconnected(&peer_id);
                                info!(peer = %peer_id, "Peer disconnected");
                                let _ = notif_tx
                                    .send(SwarmNotification::PeerDisconnected { peer_id })
                                    .await;
                            }
                        }

                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!(addr = %address, "Listening on new address");
                        }

                        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                            warn!(
                                peer = ?peer_id,
                                error = %error,
                                "Outgoing connection error"
                            );
                        }

                        SwarmEvent::IncomingConnectionError { error, .. } => {
                            warn!(error = %error, "Incoming connection error");
                        }

                        _ => {}
                    }
                }
            }
        }

        info!("Swarm event loop terminated");
    });

    Ok((cmd_tx, notif_rx, local_peer_id))
}

/// Extract a `PeerId` from a multiaddr, if one is present.
fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| {
        if let Protocol::P2p(peer_id) = p {
            Some(peer_id)
        } else {
            None
        }
    })
}
