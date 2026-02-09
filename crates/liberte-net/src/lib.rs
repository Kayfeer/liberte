//! # liberte-net
//!
//! P2P networking layer for Liberte, built on libp2p with QUIC transport.
//!
//! This crate provides:
//! - DNS-over-HTTPS resolution bypassing OS/ISP DNS
//! - Composed libp2p network behaviour (GossipSub, Kademlia, Identify, Relay, DCUtR)
//! - QUIC transport construction via `SwarmBuilder`
//! - Swarm orchestration with command/notification channels
//! - Bootstrap peer discovery via Kademlia
//! - Circuit relay v2 client logic
//! - High-level message publish/subscribe helpers
//! - Peer connection tracking

pub mod behaviour;
pub mod discovery;
pub mod dns;
pub mod messages;
pub mod peers;
pub mod relay;
pub mod swarm;
pub mod transport;

pub use behaviour::{LiberteBehaviour, LiberteEvent};
pub use discovery::load_bootstrap_peers;
pub use dns::build_doh_resolver;
pub use messages::{publish_message, subscribe_topic};
pub use peers::{ConnectionInfo, PeerTracker};
pub use relay::{request_relay_reservation, dial_via_relay};
pub use swarm::{spawn_swarm, SwarmCommand, SwarmNotification};
pub use transport::build_swarm;
