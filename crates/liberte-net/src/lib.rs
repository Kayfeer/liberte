// P2P networking layer built on libp2p with QUIC transport.

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
pub use relay::{dial_via_relay, request_relay_reservation};
pub use swarm::{spawn_swarm, SwarmCommand, SwarmNotification};
pub use transport::build_swarm;
