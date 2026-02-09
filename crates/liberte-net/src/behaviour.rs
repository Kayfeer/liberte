//! Composed libp2p `NetworkBehaviour` for the Liberte network.
//!
//! Combines GossipSub (pub/sub messaging), Kademlia (DHT peer discovery),
//! Identify (protocol negotiation), Relay client (NAT traversal via relays),
//! and DCUtR (direct connection upgrade through relay).

use libp2p::{
    dcutr,
    gossipsub,
    identify,
    kad::{self, store::MemoryStore},
    relay,
    swarm::NetworkBehaviour,
};

/// Composed network behaviour for Liberte nodes.
///
/// All sub-behaviours are driven by the single swarm event loop.
/// Construction is handled by [`super::transport::build_swarm`] via
/// `SwarmBuilder`.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "LiberteEvent")]
pub struct LiberteBehaviour {
    /// Pub/sub messaging for channel messages and signaling
    pub gossipsub: gossipsub::Behaviour,
    /// Distributed hash table for peer discovery
    pub kademlia: kad::Behaviour<MemoryStore>,
    /// Protocol identification and capability advertisement
    pub identify: identify::Behaviour,
    /// Circuit relay v2 client for NAT traversal
    pub relay_client: relay::client::Behaviour,
    /// Direct Connection Upgrade through Relay
    pub dcutr: dcutr::Behaviour,
}

/// Events emitted by the composed behaviour, one variant per sub-behaviour.
#[derive(Debug)]
pub enum LiberteEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    RelayClient(relay::client::Event),
    Dcutr(dcutr::Event),
}

impl From<gossipsub::Event> for LiberteEvent {
    fn from(event: gossipsub::Event) -> Self {
        LiberteEvent::Gossipsub(event)
    }
}

impl From<kad::Event> for LiberteEvent {
    fn from(event: kad::Event) -> Self {
        LiberteEvent::Kademlia(event)
    }
}

impl From<identify::Event> for LiberteEvent {
    fn from(event: identify::Event) -> Self {
        LiberteEvent::Identify(event)
    }
}

impl From<relay::client::Event> for LiberteEvent {
    fn from(event: relay::client::Event) -> Self {
        LiberteEvent::RelayClient(event)
    }
}

impl From<dcutr::Event> for LiberteEvent {
    fn from(event: dcutr::Event) -> Self {
        LiberteEvent::Dcutr(event)
    }
}
