//! QUIC transport configuration for libp2p.
//!
//! In libp2p 0.54 the transport is constructed internally by `SwarmBuilder`.
//! This module provides configuration helpers and the public type alias
//! that other crates can reference if they need to name the transport.

use libp2p::identity::Keypair;
use tracing::info;

/// Build a libp2p `Swarm` with QUIC transport, relay client, and the
/// Liberte composed behaviour.
///
/// This is the primary entry point that wires together:
/// - QUIC (tokio) transport for direct peer connections
/// - Relay client transport for NAT-traversed connections
/// - The [`LiberteBehaviour`](super::behaviour::LiberteBehaviour)
///
/// The returned swarm is ready to listen and dial.
///
/// # Arguments
///
/// * `keypair` - The local node's identity keypair
///
/// # Returns
///
/// A fully constructed `libp2p::Swarm<LiberteBehaviour>`.
pub fn build_swarm(
    keypair: Keypair,
) -> anyhow::Result<libp2p::Swarm<super::behaviour::LiberteBehaviour>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::Duration;

    use libp2p::gossipsub::{self, MessageAuthenticity, MessageId, ValidationMode};
    use libp2p::kad::{self, store::MemoryStore};
    use libp2p::{dcutr, identify, noise, SwarmBuilder};

    use liberte_shared::constants::{
        GOSSIPSUB_HEARTBEAT_SECS, MAX_MESSAGE_SIZE, PROTOCOL_VERSION,
    };

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_relay_client(noise::Config::new, libp2p::yamux::Config::default)?
        .with_behaviour(|key, relay_client| -> std::result::Result<super::behaviour::LiberteBehaviour, Box<dyn std::error::Error + Send + Sync>> {
            let local_peer_id = key.public().to_peer_id();

            // --- GossipSub ---
            let message_id_fn = |message: &gossipsub::Message| {
                let mut hasher = DefaultHasher::new();
                message.data.hash(&mut hasher);
                if let Some(ref source) = message.source {
                    source.hash(&mut hasher);
                }
                MessageId::from(hasher.finish().to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(GOSSIPSUB_HEARTBEAT_SECS))
                .validation_mode(ValidationMode::Strict)
                .max_transmit_size(MAX_MESSAGE_SIZE)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("GossipSub config: {e}").into()
                })?;

            let gossipsub = gossipsub::Behaviour::new(
                MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("GossipSub init: {e}").into()
            })?;

            // --- Kademlia ---
            let store = MemoryStore::new(local_peer_id);
            let mut kademlia = kad::Behaviour::new(local_peer_id, store);
            kademlia.set_mode(Some(kad::Mode::Server));

            // --- Identify ---
            let identify_config =
                identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                    .with_push_listen_addr_updates(true)
                    .with_interval(Duration::from_secs(60));
            let identify = identify::Behaviour::new(identify_config);

            // --- DCUtR ---
            let dcutr = dcutr::Behaviour::new(local_peer_id);

            Ok(super::behaviour::LiberteBehaviour {
                gossipsub,
                kademlia,
                identify,
                relay_client,
                dcutr,
            })
        })?
        .with_swarm_config(|cfg| {
            cfg.with_idle_connection_timeout(Duration::from_secs(60))
        })
        .build();

    info!(
        peer_id = %swarm.local_peer_id(),
        "Built Liberte swarm with QUIC + Relay transport"
    );

    Ok(swarm)
}
