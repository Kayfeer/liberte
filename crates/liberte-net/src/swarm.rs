use std::path::PathBuf;

use futures::StreamExt;
use libp2p::{
    gossipsub, identify, kad, multiaddr::Protocol, relay, swarm::SwarmEvent, Multiaddr, PeerId,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::behaviour::LiberteEvent;
use crate::discovery::load_bootstrap_peers;
use crate::peers::PeerTracker;
use crate::transport::build_swarm;

use liberte_shared::constants::DEFAULT_QUIC_PORT;

#[derive(Debug)]
pub enum SwarmCommand {
    Dial(Multiaddr),
    PublishMessage { topic: String, data: Vec<u8> },
    SubscribeTopic(String),
    GetPeers(tokio::sync::oneshot::Sender<Vec<PeerId>>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum SwarmNotification {
    PeerConnected {
        peer_id: PeerId,
        address: Multiaddr,
    },
    PeerDisconnected {
        peer_id: PeerId,
    },
    MessageReceived {
        source: Option<PeerId>,
        topic: String,
        data: Vec<u8>,
    },
    RelayReservation {
        relay_peer: PeerId,
        relay_addr: Multiaddr,
    },
}

pub struct SwarmConfig {
    pub bootstrap_peers_path: Option<PathBuf>,
    pub listen_port: u16,
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

pub async fn spawn_swarm(
    keypair: libp2p::identity::Keypair,
    config: SwarmConfig,
) -> anyhow::Result<(
    mpsc::Sender<SwarmCommand>,
    mpsc::Receiver<SwarmNotification>,
    PeerId,
)> {
    let mut swarm = build_swarm(keypair)?;
    let local_peer_id = *swarm.local_peer_id();

    let listen_addr_v4: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.listen_port)
        .parse()
        .expect("valid multiaddr");
    let listen_addr_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", config.listen_port)
        .parse()
        .expect("valid multiaddr");

    swarm.listen_on(listen_addr_v4)?;
    swarm.listen_on(listen_addr_v6)?;

    info!(peer_id = %local_peer_id, port = config.listen_port, "Swarm listening");

    if let Some(ref path) = config.bootstrap_peers_path {
        let bootstrap_addrs = load_bootstrap_peers(path);
        for addr in &bootstrap_addrs {
            if let Err(e) = swarm.dial(addr.clone()) {
                warn!(addr = %addr, error = %e, "Failed to dial bootstrap peer");
            } else {
                if let Some(peer_id) = extract_peer_id(addr) {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
                }
                debug!(addr = %addr, "Dialing bootstrap peer");
            }
        }

        if !bootstrap_addrs.is_empty() {
            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                warn!(error = %e, "Kademlia bootstrap failed to start");
            }
        }
    }

    for addr in &config.extra_dials {
        if let Err(e) = swarm.dial(addr.clone()) {
            warn!(addr = %addr, error = %e, "Failed to dial extra address");
        }
    }

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(256);
    let (notif_tx, notif_rx) = mpsc::channel::<SwarmNotification>(256);

    tokio::spawn(async move {
        let mut peer_tracker = PeerTracker::new();

        loop {
            tokio::select! {
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
                            info!("Command channel closed, shutting down swarm");
                            break;
                        }
                    }
                }

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

fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| {
        if let Protocol::P2p(peer_id) = p {
            Some(peer_id)
        } else {
            None
        }
    })
}
