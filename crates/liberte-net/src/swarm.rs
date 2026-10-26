use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use futures::StreamExt;
use libp2p::{
    core::transport::ListenerId,
    gossipsub, identify, kad,
    multiaddr::Protocol,
    relay,
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        ConnectionId, SwarmEvent,
    },
    Multiaddr, PeerId,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::behaviour::LiberteEvent;
use crate::discovery::{load_bootstrap_peers, resolve_bootstrap_peers};
use crate::peers::PeerTracker;
use crate::transport::build_swarm_with_options;

use liberte_shared::constants::DEFAULT_QUIC_PORT;

struct PendingDial {
    address: Multiaddr,
    reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    started: Instant,
    connection_id: ConnectionId,
}

struct ReconnectTarget {
    addresses: Vec<Multiaddr>,
    failures: u32,
    next_attempt: Instant,
}

struct RelayReservationState {
    address: Multiaddr,
    listener: Option<ListenerId>,
    failures: u32,
    next_attempt: Instant,
}

impl RelayReservationState {
    fn retry_later(&mut self) {
        self.listener = None;
        self.failures = self.failures.saturating_add(1);
        self.next_attempt =
            Instant::now() + Duration::from_secs((1u64 << self.failures.min(6)).min(60));
    }
}

fn request_reservation(
    swarm: &mut libp2p::Swarm<crate::behaviour::LiberteBehaviour>,
    reservations: &mut HashMap<PeerId, RelayReservationState>,
    peer: PeerId,
    address: Multiaddr,
) {
    if reservations.contains_key(&peer) || reservations.len() >= 128 {
        return;
    }
    let mut reservation = RelayReservationState {
        address,
        listener: None,
        failures: 0,
        next_attempt: Instant::now(),
    };
    match swarm.listen_on(reservation.address.clone()) {
        Ok(listener) => reservation.listener = Some(listener),
        Err(error) => {
            reservation.retry_later();
            warn!(%peer, %error, "Relay reservation failed; will retry");
        }
    }
    reservations.insert(peer, reservation);
}

fn remember_target(targets: &mut HashMap<PeerId, ReconnectTarget>, addresses: &[Multiaddr]) {
    let Some(peer) = addresses.first().and_then(target_peer_id) else {
        return;
    };
    if targets.len() >= 128 && !targets.contains_key(&peer) {
        return;
    }
    let target = targets.entry(peer).or_insert_with(|| ReconnectTarget {
        addresses: Vec::new(),
        failures: 0,
        next_attempt: Instant::now() + Duration::from_secs(1),
    });
    for address in addresses {
        if target_peer_id(address) == Some(peer) && !target.addresses.contains(address) {
            target.addresses.push(address.clone());
        }
    }
    if target.addresses.len() > 8 {
        target.addresses.drain(..target.addresses.len() - 8);
    }
}

#[derive(Debug)]
pub enum SwarmCommand {
    Dial(Multiaddr),
    PublishMessage {
        topic: String,
        data: Vec<u8>,
    },
    PublishConfirmed {
        topic: String,
        data: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    DialConfirmed {
        address: Multiaddr,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    DialAddressesConfirmed {
        addresses: Vec<Multiaddr>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    ListenRelay(Multiaddr),
    SubscribeTopic(String),
    SubscribeConfirmed {
        topic: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
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
        is_relayed: bool,
    },
    PeerDisconnected {
        peer_id: PeerId,
    },
    PeerSubscribed {
        peer_id: PeerId,
        topic: String,
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
    /// Upgrade relay circuits to direct connections when hole punching succeeds.
    pub enable_dcutr: bool,
    pub loopback_only: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers_path: std::env::var_os("LIBERTE_BOOTSTRAP_PEERS").map(PathBuf::from),
            listen_port: DEFAULT_QUIC_PORT,
            extra_dials: Vec::new(),
            enable_mdns: true,
            enable_dcutr: true,
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
    let mut swarm = build_swarm_with_options(keypair, config.enable_mdns, config.enable_dcutr)?;
    let local_peer_id = *swarm.local_peer_id();

    let listen_addr_v4: Multiaddr = format!(
        "/ip4/{}/udp/{}/quic-v1",
        if config.loopback_only {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        },
        config.listen_port
    )
    .parse()
    .expect("valid multiaddr");
    let mut listen_port = config.listen_port;
    match swarm.listen_on(listen_addr_v4) {
        Ok(_) => {}
        Err(_)
            if config.listen_port == DEFAULT_QUIC_PORT
                && default_port_in_use(config.loopback_only) =>
        {
            // Two profiles/processes can coexist on one device. Only an occupied
            // default port permits fallback; other listener failures stay visible.
            listen_port = 0;
            let ip = if config.loopback_only {
                "127.0.0.1"
            } else {
                "0.0.0.0"
            };
            swarm.listen_on(format!("/ip4/{ip}/udp/0/quic-v1").parse()?)?;
            info!("Default QUIC port is occupied; using an available IPv4 port");
        }
        Err(error) => return Err(error.into()),
    }
    if !config.loopback_only {
        let listen_addr_v6: Multiaddr = format!("/ip6/::/udp/{listen_port}/quic-v1").parse()?;
        if let Err(e) = swarm.listen_on(listen_addr_v6) {
            warn!(error = %e, "IPv6 unavailable; IPv4 remains active");
        }
    }

    info!(peer_id = %local_peer_id, requested_port = listen_port, "Swarm listener started");

    let bootstrap_addrs = config
        .bootstrap_peers_path
        .as_ref()
        .map(|path| load_bootstrap_peers(path))
        .unwrap_or_default();
    let bootstrap_addrs = resolve_bootstrap_peers(bootstrap_addrs).await;
    let mut reconnect_targets = HashMap::new();
    for address in bootstrap_addrs.iter().chain(&config.extra_dials).take(1024) {
        remember_target(&mut reconnect_targets, std::slice::from_ref(address));
        if let Some(peer) = target_peer_id(address) {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer, address.clone());
        } else if let Err(error) = swarm.dial(address.clone()) {
            warn!(%address, %error, "Initial peer connection failed");
        }
    }
    reconnect_targets.remove(&local_peer_id);
    for (peer, target) in &reconnect_targets {
        let options = DialOpts::peer_id(*peer)
            .addresses(target.addresses.clone())
            .build();
        if let Err(error) = swarm.dial(options) {
            warn!(%peer, %error, "Initial peer connection failed; will retry");
        }
    }
    if !bootstrap_addrs.is_empty() {
        if let Err(error) = swarm.behaviour_mut().kademlia.bootstrap() {
            warn!(%error, "Kademlia bootstrap failed to start");
        }
    }

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(256);
    let (notif_tx, notif_rx) = mpsc::channel::<SwarmNotification>(256);

    tokio::spawn(async move {
        let mut peer_tracker = PeerTracker::new();
        let mut pending_dials: Vec<PendingDial> = Vec::new();
        let mut maintenance = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut relay_reservations: HashMap<PeerId, RelayReservationState> = HashMap::new();

        loop {
            tokio::select! {
                _ = maintenance.tick() => {
                    let mut index = 0;
                    while index < pending_dials.len() {
                        if pending_dials[index].started.elapsed() >= std::time::Duration::from_secs(12) {
                            let PendingDial { reply, .. } = pending_dials.swap_remove(index);
                            let _ = reply.send(Err("Connection timed out".into()));
                        } else { index += 1; }
                    }
                    let now = Instant::now();
                    let mut attempts = 0;
                    for (peer, target) in &mut reconnect_targets {
                        if attempts >= 4 { break; }
                        if swarm.is_connected(peer) || target.next_attempt > now { continue; }
                        attempts += 1;
                        target.failures = target.failures.saturating_add(1);
                        target.next_attempt = now + Duration::from_secs((1u64 << target.failures.min(6)).min(60));
                        let options = DialOpts::peer_id(*peer).addresses(target.addresses.clone()).build();
                        if let Err(error) = swarm.dial(options) {
                            debug!(%peer, %error, "Reconnect attempt deferred");
                        }
                    }
                    let mut attempts = 0;
                    for (peer, reservation) in &mut relay_reservations {
                        if attempts >= 4 { break; }
                        if reservation.listener.is_some() || reservation.next_attempt > now || !swarm.is_connected(peer) { continue; }
                        attempts += 1;
                        match swarm.listen_on(reservation.address.clone()) {
                            Ok(listener) => reservation.listener = Some(listener),
                            Err(error) => {
                                reservation.retry_later();
                                debug!(%peer, %error, "Relay reservation retry deferred");
                            }
                        }
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(SwarmCommand::Dial(addr)) => {
                            remember_target(&mut reconnect_targets, std::slice::from_ref(&addr));
                            if let Err(e) = swarm.dial(addr.clone()) {
                                error!(addr = %addr, error = %e, "Dial failed");
                            }
                        }
                        Some(SwarmCommand::DialConfirmed { address, reply }) => {
                            remember_target(&mut reconnect_targets, std::slice::from_ref(&address));
                            if target_peer_id(&address).is_some_and(|p| peer_tracker.is_connected(&p)) {
                                let _ = reply.send(Ok(()));
                            } else if pending_dials.len() >= 32 {
                                let _ = reply.send(Err("Too many pending connections".into()));
                            } else {
                                let options = DialOpts::from(address.clone());
                                let connection_id = options.connection_id();
                                match swarm.dial(options) {
                                    Ok(()) => pending_dials.push(PendingDial { address, reply, started: Instant::now(), connection_id }),
                                    Err(e) => { let _ = reply.send(Err(e.to_string())); }
                                }
                            }
                        }
                        Some(SwarmCommand::DialAddressesConfirmed { addresses, reply }) => {
                            let peer = addresses.first().and_then(target_peer_id);
                            if addresses.is_empty() || addresses.len() > 8 || peer.is_none()
                                || addresses.iter().any(|a| target_peer_id(a) != peer) {
                                let _ = reply.send(Err("Expected 1 to 8 addresses for the same destination peer".into()));
                                continue;
                            }
                            let peer = peer.expect("validated destination peer");
                            remember_target(&mut reconnect_targets, &addresses);
                            if swarm.is_connected(&peer) {
                                let _ = reply.send(Ok(()));
                            } else if pending_dials.len() >= 32 {
                                let _ = reply.send(Err("Too many pending connections".into()));
                            } else {
                                let first = addresses[0].clone();
                                let options = DialOpts::peer_id(peer).condition(PeerCondition::Disconnected)
                                    .addresses(addresses).build();
                                let connection_id = options.connection_id();
                                match swarm.dial(options) {
                                    Ok(()) => pending_dials.push(PendingDial { address: first, reply, started: Instant::now(), connection_id }),
                                    Err(error) => { let _ = reply.send(Err(error.to_string())); }
                                }
                            }
                        }
                        Some(SwarmCommand::ListenRelay(address)) => {
                            let mut relay_address = address.clone();
                            if matches!(relay_address.pop(), Some(Protocol::P2pCircuit)) {
                                if let Some(peer) = target_peer_id(&relay_address) {
                                    request_reservation(&mut swarm, &mut relay_reservations, peer, address);
                                }
                            }
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
                                connection_mode: if peer_tracker.relayed_count() > 0 { "relayed" } else if peer_tracker.direct_count() > 0 { "direct" } else { "disconnected" }.into(),
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
                            gossipsub::Event::Subscribed { peer_id, topic },
                        )) => {
                            let _ = notif_tx.send(SwarmNotification::PeerSubscribed {
                                peer_id,
                                topic: topic.to_string(),
                            }).await;
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

                        SwarmEvent::Behaviour(LiberteEvent::Kademlia(event)) => {
                            if let kad::Event::OutboundQueryProgressed { result, .. } = *event {
                                debug!(result = ?result, "Kademlia query progressed");
                            }
                        }

                        SwarmEvent::Behaviour(LiberteEvent::Identify(event)) => {
                            let identify::Event::Received { peer_id, info, .. } = *event else { continue; };
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
                            if supports_relay {
                                if let Some(connection) = peer_tracker.get(&peer_id) {
                                    let mut circuit = connection.address.clone();
                                    if !matches!(circuit.iter().last(), Some(Protocol::P2p(_))) { circuit.push(Protocol::P2p(peer_id)); }
                                    circuit.push(Protocol::P2pCircuit);
                                    request_reservation(&mut swarm, &mut relay_reservations, peer_id, circuit);
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
                            if let Some(reservation) = relay_reservations.get_mut(&relay_peer_id) {
                                reservation.failures = 0;
                            }
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
                            // For inbound circuits, send_back_addr may contain only the
                            // remote PeerId. The local endpoint carries the relay route.
                            let is_relayed = endpoint.is_relayed();
                            peer_tracker.on_connected(peer_id, addr.clone(), is_relayed);
                            if let Some(target) = reconnect_targets.get_mut(&peer_id) {
                                target.failures = 0;
                                target.next_attempt = Instant::now() + Duration::from_secs(1);
                            }
                            let mut index = 0;
                            while index < pending_dials.len() {
                                let requested = &pending_dials[index].address;
                                let matches = target_peer_id(requested).map_or_else(
                                    || requested == &addr || addr.to_string().starts_with(&requested.to_string()),
                                    |expected| expected == peer_id);
                                if matches {
                                    let PendingDial { reply, .. } = pending_dials.swap_remove(index);
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
                                    is_relayed,
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
                                if let Some(target) = reconnect_targets.get_mut(&peer_id) {
                                    target.next_attempt = Instant::now() + Duration::from_secs(1);
                                }
                                if let Some(reservation) = relay_reservations.remove(&peer_id) {
                                    if let Some(listener) = reservation.listener { swarm.remove_listener(listener); }
                                }
                                info!(peer = %peer_id, "Peer disconnected");
                                let _ = notif_tx
                                    .send(SwarmNotification::PeerDisconnected { peer_id })
                                    .await;
                            }
                        }

                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!(addr = %address, "Listening on new address");
                        }

                        SwarmEvent::ListenerClosed { listener_id, reason, .. } => {
                            if let Some((peer, reservation)) = relay_reservations.iter_mut()
                                .find(|(_, reservation)| reservation.listener == Some(listener_id)) {
                                reservation.retry_later();
                                warn!(%peer, ?reason, "Relay listener closed; reservation will retry");
                            }
                        }

                        SwarmEvent::ListenerError { listener_id, error } => {
                            // ListenerError is explicitly non-terminal. Creating another
                            // reservation here would duplicate a still-active listener.
                            warn!(?listener_id, %error, "Listener reported a recoverable error");
                        }

                        SwarmEvent::OutgoingConnectionError { peer_id, connection_id, error } => {
                            let mut index = 0;
                            while index < pending_dials.len() {
                                if pending_dials[index].connection_id == connection_id {
                                    let PendingDial { reply, .. } = pending_dials.swap_remove(index);
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

fn default_port_in_use(loopback_only: bool) -> bool {
    // Transport wrappers can erase io::ErrorKind. After a failed listen, ask
    // the OS directly whether this exact IPv4 endpoint is occupied. Other
    // errors (permissions, unavailable interfaces) must not trigger fallback.
    let ip = if loopback_only {
        std::net::Ipv4Addr::LOCALHOST
    } else {
        std::net::Ipv4Addr::UNSPECIFIED
    };
    matches!(std::net::UdpSocket::bind((ip, DEFAULT_QUIC_PORT)), Err(error) if error.kind() == std::io::ErrorKind::AddrInUse)
}

/// The terminal /p2p component is the destination; earlier IDs may identify relays.
pub fn target_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    match addr.iter().last() {
        Some(Protocol::P2p(peer)) => Some(peer),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relayed_address_targets_the_destination_not_the_relay() {
        let relay = PeerId::random();
        let target = PeerId::random();
        let circuit: Multiaddr = format!("/ip4/127.0.0.1/udp/4001/quic-v1/p2p/{relay}/p2p-circuit")
            .parse()
            .unwrap();
        assert_eq!(target_peer_id(&circuit), None);
        assert_eq!(
            target_peer_id(&circuit.with(Protocol::P2p(target))),
            Some(target)
        );
    }
}
