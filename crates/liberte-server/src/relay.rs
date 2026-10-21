use std::{collections::HashSet, path::Path, time::Duration};

use futures::StreamExt;
use libp2p::{
    identify, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, SwarmBuilder,
};
use tracing::{debug, info, warn};

use liberte_shared::constants::PROTOCOL_VERSION;
use crate::{blob_store::atomic_write, config::ServerConfig};

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
    fn from(event: relay::Event) -> Self { Self::Relay(event) }
}

impl From<identify::Event> for RelayServerEvent {
    fn from(event: identify::Event) -> Self { Self::Identify(Box::new(event)) }
}

async fn load_identity(path: &Path) -> anyhow::Result<libp2p::identity::Keypair> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => {
            anyhow::ensure!(!metadata.file_type().is_symlink(), "Relay identity must not be a symlink");
            let bytes = tokio::fs::read(path).await?;
            Ok(libp2p::identity::Keypair::from_protobuf_encoding(&bytes)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let key = libp2p::identity::Keypair::generate_ed25519();
            // NamedTempFile uses private permissions on Unix; deploy the data
            // directory with an account-only ACL on Windows.
            atomic_write(path.to_owned(), key.to_protobuf_encoding()?).await?;
            Ok(key)
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn spawn_relay(config: &ServerConfig) -> anyhow::Result<PeerId> {
    let keypair = load_identity(&config.blob_storage_path.join("relay-identity.key")).await?;
    let local_peer_id = keypair.public().to_peer_id();
    let max_peers = config.max_peers;
    info!(peer_id = %local_peer_id, "Starting relay server");

    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(move |key| {
            let relay_config = relay::Config {
                max_reservations: max_peers,
                max_reservations_per_peer: 1,
                max_circuits: max_peers.saturating_mul(2),
                max_circuits_per_peer: 4,
                max_circuit_duration: Duration::from_secs(3600),
                max_circuit_bytes: 100 * 1024 * 1024,
                ..relay::Config::default()
            };
            let relay = relay::Behaviour::new(key.public().to_peer_id(), relay_config);
            let identify = identify::Behaviour::new(
                identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                    .with_push_listen_addr_updates(true)
                    .with_interval(Duration::from_secs(60)),
            );
            Ok(RelayServerBehaviour { relay, identify })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();

    let listen: Multiaddr = config.listen_addr.parse()?;
    swarm.listen_on(listen)?;
    if let Some(address) = &config.public_relay_addr {
        swarm.add_external_address(address.parse()?);
    } else {
        warn!("PUBLIC_RELAY_ADDR is unset: internet relay reservations require a reachable announced address");
    }

    tokio::spawn(async move {
        let mut connected = HashSet::new();
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(RelayServerEvent::Relay(event)) => {
                    debug!(?event, "Relay event");
                }
                SwarmEvent::Behaviour(RelayServerEvent::Identify(event)) => {
                    if let identify::Event::Received { peer_id, .. } = *event {
                        debug!(peer = %peer_id, "Peer identified");
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!(addr = %address, "Relay listening");
                }
                SwarmEvent::ConnectionEstablished { peer_id, connection_id, num_established, .. } => {
                    if !connected.contains(&peer_id) && connected.len() >= max_peers {
                        let _ = swarm.disconnect_peer_id(peer_id);
                    } else if num_established.get() > 2 {
                        swarm.close_connection(connection_id);
                    } else {
                        connected.insert(peer_id);
                    }
                }
                SwarmEvent::ConnectionClosed { peer_id, num_established: 0, .. } => {
                    connected.remove(&peer_id);
                }
                SwarmEvent::OutgoingConnectionError { error, .. } => {
                    debug!(%error, "Outgoing relay connection failed");
                }
                SwarmEvent::IncomingConnectionError { error, .. } => {
                    debug!(%error, "Relay connection failed");
                }
                _ => {}
            }
        }
    });
    Ok(local_peer_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn relay_peer_id_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("relay-identity.key");
        let first = load_identity(&path).await.unwrap().public().to_peer_id();
        let second = load_identity(&path).await.unwrap().public().to_peer_id();
        assert_eq!(first, second);
    }
}
