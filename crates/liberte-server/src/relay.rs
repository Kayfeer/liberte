//! libp2p relay server setup.
//!
//! Builds a minimal swarm that acts as a circuit relay v2 **server**.
//! Peers behind NAT can request relay reservations so that other peers
//! can reach them through this server.

use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    identify, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, SwarmBuilder,
};
use tracing::{debug, info, warn};

use liberte_shared::constants::PROTOCOL_VERSION;

// ---------------------------------------------------------------------------
// Relay server behaviour
// ---------------------------------------------------------------------------

/// Composed `NetworkBehaviour` for the relay server.
///
/// Only includes the relay (server-side) behaviour and Identify so that
/// connecting clients can negotiate protocols. The server does NOT
/// participate in GossipSub or Kademlia -- it is a dedicated relay.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "RelayServerEvent")]
pub struct RelayServerBehaviour {
    /// Circuit relay v2 server behaviour.
    pub relay: relay::Behaviour,
    /// Identify protocol for capability advertisement.
    pub identify: identify::Behaviour,
}

/// Events emitted by the relay server behaviour.
#[derive(Debug)]
pub enum RelayServerEvent {
    Relay(relay::Event),
    Identify(identify::Event),
}

impl From<relay::Event> for RelayServerEvent {
    fn from(event: relay::Event) -> Self {
        RelayServerEvent::Relay(event)
    }
}

impl From<identify::Event> for RelayServerEvent {
    fn from(event: identify::Event) -> Self {
        RelayServerEvent::Identify(event)
    }
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Spawn the libp2p relay server as a background tokio task.
///
/// The relay listens on QUIC at the given multiaddr string and accepts
/// relay reservations from any connecting peer (rate-limited by libp2p
/// defaults).
///
/// Returns the local `PeerId` so that clients can address the relay.
pub async fn spawn_relay(listen_addr: &str) -> anyhow::Result<PeerId> {
    // Generate a server identity (ephemeral for now; in production this
    // should be loaded from a persisted key file).
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = keypair.public().to_peer_id();

    info!(peer_id = %local_peer_id, "Starting relay server");

    // --- Build swarm using SwarmBuilder (libp2p 0.54 pattern) ---
    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let peer_id = key.public().to_peer_id();

            // Relay server behaviour
            let relay_config = relay::Config::default();
            let relay_behaviour = relay::Behaviour::new(peer_id, relay_config);

            // Identify behaviour
            let identify_config =
                identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                    .with_push_listen_addr_updates(true)
                    .with_interval(Duration::from_secs(60));
            let identify_behaviour = identify::Behaviour::new(identify_config);

            Ok(RelayServerBehaviour {
                relay: relay_behaviour,
                identify: identify_behaviour,
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();

    // Listen on the configured multiaddr
    let multiaddr: Multiaddr = listen_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid listen multiaddr '{}': {}", listen_addr, e))?;

    swarm.listen_on(multiaddr.clone())?;
    info!(addr = %multiaddr, "Relay server listening");

    // Spawn the event loop
    tokio::spawn(async move {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(RelayServerEvent::Relay(event)) => {
                    match &event {
                        relay::Event::ReservationReqAccepted {
                            src_peer_id,
                            ..
                        } => {
                            info!(
                                peer = %src_peer_id,
                                "Relay reservation accepted"
                            );
                        }
                        relay::Event::ReservationTimedOut { src_peer_id, .. } => {
                            debug!(
                                peer = %src_peer_id,
                                "Relay reservation timed out"
                            );
                        }
                        relay::Event::CircuitReqDenied {
                            src_peer_id,
                            dst_peer_id,
                            ..
                        } => {
                            debug!(
                                src = %src_peer_id,
                                dst = %dst_peer_id,
                                "Circuit request denied"
                            );
                        }
                        relay::Event::CircuitReqAccepted {
                            src_peer_id,
                            dst_peer_id,
                            ..
                        } => {
                            info!(
                                src = %src_peer_id,
                                dst = %dst_peer_id,
                                "Circuit relay established"
                            );
                        }
                        relay::Event::CircuitClosed {
                            src_peer_id,
                            dst_peer_id,
                            ..
                        } => {
                            debug!(
                                src = %src_peer_id,
                                dst = %dst_peer_id,
                                "Circuit relay closed"
                            );
                        }
                        _ => {
                            debug!(event = ?event, "Relay event");
                        }
                    }
                }

                SwarmEvent::Behaviour(RelayServerEvent::Identify(
                    identify::Event::Received { peer_id, info, .. },
                )) => {
                    debug!(
                        peer = %peer_id,
                        protocol = ?info.protocol_version,
                        "Identify: received info from peer"
                    );
                }

                SwarmEvent::NewListenAddr { address, .. } => {
                    info!(addr = %address, "Relay server listening on new address");
                }

                SwarmEvent::ConnectionEstablished {
                    peer_id, endpoint, ..
                } => {
                    debug!(
                        peer = %peer_id,
                        addr = %endpoint.get_remote_address(),
                        "Peer connected to relay"
                    );
                }

                SwarmEvent::ConnectionClosed {
                    peer_id,
                    num_established,
                    ..
                } => {
                    if num_established == 0 {
                        debug!(peer = %peer_id, "Peer fully disconnected from relay");
                    }
                }

                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    warn!(peer = ?peer_id, error = %error, "Outgoing connection error");
                }

                SwarmEvent::IncomingConnectionError { error, .. } => {
                    warn!(error = %error, "Incoming connection error");
                }

                _ => {}
            }
        }
    });

    Ok(local_peer_id)
}
