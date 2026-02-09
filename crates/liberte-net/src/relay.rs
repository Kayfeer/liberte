use libp2p::{Multiaddr, PeerId};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::swarm::SwarmCommand;

pub async fn request_relay_reservation(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    relay_peer_id: &PeerId,
    relay_addr: &Multiaddr,
) -> anyhow::Result<()> {
    // <relay_addr>/p2p/<relay_peer_id>/p2p-circuit
    let circuit_addr = relay_addr
        .clone()
        .with(libp2p::multiaddr::Protocol::P2p(*relay_peer_id))
        .with(libp2p::multiaddr::Protocol::P2pCircuit);

    info!(
        relay = %relay_peer_id,
        addr = %circuit_addr,
        "Requesting relay reservation"
    );

    cmd_tx
        .send(SwarmCommand::Dial(relay_addr.clone()))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    // Reservation happens when the swarm processes listen_on for the circuit addr
    cmd_tx
        .send(SwarmCommand::Dial(circuit_addr))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    Ok(())
}

pub async fn dial_via_relay(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    relay_addr: &Multiaddr,
    relay_peer_id: &PeerId,
    target_peer_id: &PeerId,
) -> anyhow::Result<()> {
    // <relay_addr>/p2p/<relay_peer_id>/p2p-circuit/p2p/<target_peer_id>
    let relayed_addr = relay_addr
        .clone()
        .with(libp2p::multiaddr::Protocol::P2p(*relay_peer_id))
        .with(libp2p::multiaddr::Protocol::P2pCircuit)
        .with(libp2p::multiaddr::Protocol::P2p(*target_peer_id));

    debug!(
        relay = %relay_peer_id,
        target = %target_peer_id,
        addr = %relayed_addr,
        "Dialing peer via relay"
    );

    cmd_tx
        .send(SwarmCommand::Dial(relayed_addr))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    Ok(())
}

pub fn build_relayed_addr(
    relay_addr: &Multiaddr,
    relay_peer_id: &PeerId,
    target_peer_id: &PeerId,
) -> Multiaddr {
    relay_addr
        .clone()
        .with(libp2p::multiaddr::Protocol::P2p(*relay_peer_id))
        .with(libp2p::multiaddr::Protocol::P2pCircuit)
        .with(libp2p::multiaddr::Protocol::P2p(*target_peer_id))
}

pub fn build_circuit_addr(relay_addr: &Multiaddr, relay_peer_id: &PeerId) -> Multiaddr {
    relay_addr
        .clone()
        .with(libp2p::multiaddr::Protocol::P2p(*relay_peer_id))
        .with(libp2p::multiaddr::Protocol::P2pCircuit)
}
