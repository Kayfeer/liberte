use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    identify, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, SwarmBuilder,
};
use tracing::{debug, info, warn};

use liberte_shared::constants::PROTOCOL_VERSION;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "RelayServerEvent")]
pub struct RelayServerBehaviour {
    pub relay: relay::Behaviour,
    pub identify: identify::Behaviour,
}

#[derive(Debug)]
pub enum RelayServerEvent {
    Relay(relay::Event),
    Identify(Box<identify::Event>),
}

impl From<relay::Event> for RelayServerEvent {
    fn from(event: relay::Event) -> Self {
        RelayServerEvent::Relay(event)
    }
}

impl From<identify::Event> for RelayServerEvent {
    fn from(event: identify::Event) -> Self {
        RelayServerEvent::Identify(Box::new(event))
    }
}

pub async fn spawn_relay(listen_addr: &str) -> anyhow::Result<PeerId> {
    // TODO: persist keypair to disk for production
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = keypair.public().to_peer_id();

    info!(peer_id = %local_peer_id, "Starting relay server");

    let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_quic()
        .with_behaviour(|key| {
            let peer_id = key.public().to_peer_id();

            let relay_config = relay::Config::default();
            let relay_behaviour = relay::Behaviour::new(peer_id, relay_config);

            let identify_config = identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
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

    let multiaddr: Multiaddr = listen_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid listen multiaddr '{}': {}", listen_addr, e))?;

    swarm.listen_on(multiaddr.clone())?;
    info!(addr = %multiaddr, "Relay server listening");

    tokio::spawn(async move {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(RelayServerEvent::Relay(event)) => match &event {
                    relay::Event::ReservationReqAccepted { src_peer_id, .. } => {
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
                },

                SwarmEvent::Behaviour(RelayServerEvent::Identify(event)) => {
                    if let identify::Event::Received { peer_id, info, .. } = *event {
                        debug!(
                            peer = %peer_id,
                            protocol = ?info.protocol_version,
                            "Identify: received info from peer"
                        );
                    }
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
