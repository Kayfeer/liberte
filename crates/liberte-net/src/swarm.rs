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
use crate::transport::build_swarm_with_discovery;

use liberte_shared::constants::DEFAULT_QUIC_PORT;

#[derive(Debug)]
pub enum SwarmCommand {
    Dial(Multiaddr),
    PublishMessage { topic: String, data: Vec<u8> },
    PublishConfirmed { topic: String, data: Vec<u8>, reply: tokio::sync::oneshot::Sender<Result<(), String>> },
    DialConfirmed { address: Multiaddr, reply: tokio::sync::oneshot::Sender<Result<(), String>> },
    ListenRelay(Multiaddr),
    SubscribeTopic(String),
    SubscribeConfirmed { topic: String, reply: tokio::sync::oneshot::Sender<Result<(), String>> },
    GetPeers(tokio::sync::oneshot::Sender<Vec<PeerId>>),
    GetSnapshot(tokio::sync::oneshot::Sender<NetworkSnapshot>),
    Shutdown,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    pub peers: Vec<String>,
    pub connection_mode: String,
    pub listen_addresses: Vec<String>,
    pub external_addresses: Vec<String>,
    pub local_peer_id: String,
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
    pub enable_mdns: bool,
    pub loopback_only: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers_path: std::env::var_os("LIBERTE_BOOTSTRAP_PEERS").map(PathBuf::from),
            listen_port: DEFAULT_QUIC_PORT,
            extra_dials: Vec::new(),
            enable_mdns: true,
            loopback_only: false,
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
    let mut swarm = build_swarm_with_discovery(keypair, config.enable_mdns)?;
    let local_peer_id = *swarm.local_peer_id();

    let listen_addr_v4: Multiaddr = format!("/ip4/{}/udp/{}/quic-v1", if config.loopback_only { "127.0.0.1" } else { "0.0.0.0" }, config.listen_port)
        .parse()
        .expect("valid multiaddr");
    let listen_addr_v6: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", config.listen_port)
        .parse()
        .expect("valid multiaddr");

    swarm.listen_on(listen_addr_v4)?;
    if !config.loopback_only {
        if let Err(e) = swarm.listen_on(listen_addr_v6) { warn!(error = %e, "IPv6 unavailable; IPv4 remains active"); }
    }

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
        let mut pending_dials: Vec<(Multiaddr, tokio::sync::oneshot::Sender<Result<(), String>>, std::time::Instant)> = Vec::new();
        let mut maintenance = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut relay_reservations = std::collections::HashSet::new();

        loop {
            tokio::select! {
                _ = maintenance.tick() => {
                    let mut index = 0;
                    while index < pending_dials.len() {
                        if pending_dials[index].2.elapsed() >= std::time::Duration::from_secs(12) {
                            let (_, reply, _) = pending_dials.swap_remove(index);
                            let _ = reply.send(Err("Connection timed out".into()));
                        } else { index += 1; }
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(SwarmCommand::Dial(addr)) => {
                            if let Err(e) = swarm.dial(addr.clone()) {
                                error!(addr = %addr, error = %e, "Dial failed");
                            }
                        }
                        Some(SwarmCommand::DialConfirmed { address, reply }) => {
                            if extract_peer_id(&address).is_some_and(|p| peer_tracker.is_connected(&p)) {
                                let _ = reply.send(Ok(()));
                            } else if pending_dials.len() >= 32 {
                                let _ = reply.send(Err("Too many pending connections".into()));
                            } else {
                                match swarm.dial(address.clone()) {
                                    Ok(()) => pending_dials.push((address, reply, std::time::Instant::now())),
                                    Err(e) => { let _ = reply.send(Err(e.to_string())); }
                                }
                            }
                        }
                        Some(SwarmCommand::ListenRelay(address)) => {
                            if let Err(e) = swarm.listen_on(address) { warn!(error=%e, "Relay reservation failed"); }
                        }
                        Some(SwarmCommand::PublishConfirmed { topic, data, reply }) => {
                            let result = swarm.behaviour_mut().gossipsub.publish(gossipsub::IdentTopic::new(topic), data)
                                .map(|_| ()).map_err(|e| e.to_string());
                            let _ = reply.send(result);
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
                        Some(SwarmCommand::SubscribeConfirmed { topic, reply }) => {
                            let result = swarm.behaviour_mut().gossipsub.subscribe(&gossipsub::IdentTopic::new(topic))
                                .map(|_| ()).map_err(|e| e.to_string());
                            let _ = reply.send(result);
                        }
                        Some(SwarmCommand::GetSnapshot(reply)) => {
                            let append_peer = |address: &Multiaddr| {
                                if matches!(address.iter().last(), Some(Protocol::P2p(peer)) if peer == local_peer_id) { address.to_string() }
                                else { address.clone().with(Protocol::P2p(local_peer_id)).to_string() }
                            };
                            let snapshot = NetworkSnapshot {
                                peers: peer_tracker.connected_peers().iter().map(ToString::to_string).collect(),
                                connection_mode: if peer_tracker.direct_count() > 0 { "direct" } else if peer_tracker.relayed_count() > 0 { "relayed" } else { "disconnected" }.into(),
                                listen_addresses: swarm.listeners().map(append_peer).collect(),
                                external_addresses: swarm.external_addresses().map(append_peer).collect(),
                                local_peer_id: local_peer_id.to_string(),
                            };
                            let _ = reply.send(snapshot);
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
                        SwarmEvent::Behaviour(LiberteEvent::Mdns(libp2p::mdns::Event::Discovered(peers))) => {
                            for (peer, address) in peers {
                                swarm.behaviour_mut().kademlia.add_address(&peer, address.clone());
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                                if peer != local_peer_id && !swarm.is_connected(&peer) {
                                    let address = if matches!(address.iter().last(), Some(Protocol::P2p(_))) { address } else { address.with(Protocol::P2p(peer)) };
                                    let _ = swarm.dial(address);
                                }
                            }
                        }
                        SwarmEvent::Behaviour(LiberteEvent::Mdns(libp2p::mdns::Event::Expired(peers))) => {
                            for (peer, _) in peers { swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer); }
                        }
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
                            let supports_relay = info.protocols.iter().any(|p| {
                                let protocol = p.to_string();
                                protocol.starts_with("/libp2p/circuit/relay/") && protocol.ends_with("/hop")
                            });
                            if supports_relay && relay_reservations.insert(peer_id) {
                                if let Some(connection) = peer_tracker.get(&peer_id) {
                                    let mut circuit = connection.address.clone();
                                    if !matches!(circuit.iter().last(), Some(Protocol::P2p(_))) { circuit.push(Protocol::P2p(peer_id)); }
                                    circuit.push(Protocol::P2pCircuit);
                                    if let Err(e) = swarm.listen_on(circuit) {
                                        relay_reservations.remove(&peer_id);
                                        warn!(error=%e, "Could not reserve relay circuit");
                                    }
                                }
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
                            let mut index = 0;
                            while index < pending_dials.len() {
                                let requested = &pending_dials[index].0;
                                let matches = extract_peer_id(requested).map_or_else(
                                    || requested == &addr || addr.to_string().starts_with(&requested.to_string()),
                                    |expected| expected == peer_id);
                                if matches {
                                    let (_, reply, _) = pending_dials.swap_remove(index);
                                    let _ = reply.send(Ok(()));
                                } else { index += 1; }
                            }

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
                                relay_reservations.remove(&peer_id);
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
                            let mut index = 0;
                            while index < pending_dials.len() {
                                if peer_id.is_some() && extract_peer_id(&pending_dials[index].0) == peer_id {
                                    let (_, reply, _) = pending_dials.swap_remove(index);
                                    let _ = reply.send(Err(error.to_string()));
                                } else { index += 1; }
                            }
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

