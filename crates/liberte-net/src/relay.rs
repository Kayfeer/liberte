//! Circuit relay v2 client helpers.
//!
//! Provides convenience functions for requesting a relay reservation (so other
//! peers can reach us through the relay) and for dialing a remote peer via
//! a relay node.

use libp2p::{Multiaddr, PeerId};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::swarm::SwarmCommand;

/// Request a relay reservation through the given relay node.
///
/// This instructs the swarm to listen on the relay's circuit address so that
/// other peers behind NAT can reach us via the relay.
///
/// # Arguments
///
/// * `cmd_tx` - The swarm command channel sender
/// * `relay_peer_id` - The PeerId of the relay node
/// * `relay_addr` - The base multiaddr of the relay node (without `/p2p-circuit`)
///
/// # Returns
///
/// `Ok(())` if the listen command was sent, or an error if the channel is closed.
pub async fn request_relay_reservation(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    relay_peer_id: &PeerId,
    relay_addr: &Multiaddr,
) -> anyhow::Result<()> {
    // Construct the circuit relay listen address:
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

    // First, ensure we are connected to the relay
    cmd_tx
        .send(SwarmCommand::Dial(relay_addr.clone()))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    // The actual reservation happens when the swarm processes the
    // listen_on for the circuit address. We model this as a Dial command
    // to the circuit address, which triggers the relay client behaviour.
    cmd_tx
        .send(SwarmCommand::Dial(circuit_addr))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    Ok(())
}

/// Dial a remote peer through a relay node.
///
/// Constructs the relayed address and sends a Dial command to the swarm.
///
/// # Arguments
///
/// * `cmd_tx` - The swarm command channel sender
/// * `relay_addr` - The base multiaddr of the relay node
/// * `relay_peer_id` - The relay node's PeerId
/// * `target_peer_id` - The PeerId of the peer we want to reach
///
/// # Returns
///
/// `Ok(())` if the dial command was sent.
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

/// Build a relayed multiaddr from components.
///
/// Returns: `<relay_addr>/p2p/<relay_peer_id>/p2p-circuit/p2p/<target_peer_id>`
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

/// Build a circuit listen address for a relay.
///
/// Returns: `<relay_addr>/p2p/<relay_peer_id>/p2p-circuit`
pub fn build_circuit_addr(relay_addr: &Multiaddr, relay_peer_id: &PeerId) -> Multiaddr {
    relay_addr
        .clone()
        .with(libp2p::multiaddr::Protocol::P2p(*relay_peer_id))
        .with(libp2p::multiaddr::Protocol::P2pCircuit)
}
