use libp2p::{
    dcutr, gossipsub, identify,
    kad::{self, store::MemoryStore},
    mdns, relay,
    swarm::NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "LiberteEvent")]
pub struct LiberteBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub mdns: libp2p::swarm::behaviour::toggle::Toggle<mdns::tokio::Behaviour>,
}

#[derive(Debug)]
pub enum LiberteEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    RelayClient(relay::client::Event),
    Dcutr(dcutr::Event),
    Mdns(mdns::Event),
}

impl From<mdns::Event> for LiberteEvent {
    fn from(event: mdns::Event) -> Self {
        Self::Mdns(event)
    }
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
