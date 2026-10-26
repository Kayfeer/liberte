use libp2p::identity::Keypair;
use tracing::{info, warn};

pub fn build_swarm(
    keypair: Keypair,
) -> anyhow::Result<libp2p::Swarm<super::behaviour::LiberteBehaviour>> {
    build_swarm_with_discovery(keypair, true)
}

pub fn build_swarm_with_discovery(
    keypair: Keypair,
    enable_mdns: bool,
) -> anyhow::Result<libp2p::Swarm<super::behaviour::LiberteBehaviour>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::Duration;

    use libp2p::gossipsub::{self, MessageAuthenticity, MessageId, ValidationMode};
    use libp2p::kad::{self, store::MemoryStore};
    use libp2p::{dcutr, identify, noise, SwarmBuilder};

    use liberte_shared::constants::{GOSSIPSUB_HEARTBEAT_SECS, MAX_MESSAGE_SIZE, PROTOCOL_VERSION};

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_relay_client(noise::Config::new, libp2p::yamux::Config::default)?
        .with_behaviour(
            move |key,
             relay_client|
             -> std::result::Result<
                super::behaviour::LiberteBehaviour,
                Box<dyn std::error::Error + Send + Sync>,
            > {
                let local_peer_id = key.public().to_peer_id();

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
                    // Initial peer handshakes include every subscribed channel.
                    .max_control_message_size(64 * 1024)
                    .message_id_fn(message_id_fn)
                    .build()
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        format!("GossipSub config: {e}").into()
                    })?;

                // Keep subscription memory bounded while supporting profiles with
                // more than the new upstream default of 100 channels.
                let subscription_filter = gossipsub::MaxCountSubscriptionFilter {
                    filter: gossipsub::AllowAllSubscriptionFilter {},
                    max_subscribed_topics: 1024,
                    max_subscriptions_per_request: 1024,
                };
                let gossipsub = gossipsub::Behaviour::new_with_subscription_filter(
                    MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                    subscription_filter,
                )
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("GossipSub init: {e}").into()
                })?;

                let store = MemoryStore::new(local_peer_id);
                let mut kademlia = kad::Behaviour::new(local_peer_id, store);
                kademlia.set_mode(Some(kad::Mode::Server));

                let identify_config =
                    identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                        .with_push_listen_addr_updates(true)
                        .with_interval(Duration::from_secs(60));
                let identify = identify::Behaviour::new(identify_config);

                let dcutr = dcutr::Behaviour::new(local_peer_id);
                let mdns = if enable_mdns {
                    match libp2p::mdns::tokio::Behaviour::new(libp2p::mdns::Config::default(), local_peer_id) {
                        Ok(discovery) => Some(discovery),
                        Err(error) => { warn!(%error, "LAN discovery unavailable; explicit connections remain available"); None }
                    }
                } else { None };

                Ok(super::behaviour::LiberteBehaviour {
                    gossipsub,
                    kademlia,
                    identify,
                    relay_client,
                    dcutr,
                    mdns: mdns.into(),
                })
            },
        )?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    info!(
        peer_id = %swarm.local_peer_id(),
        "Built Liberte swarm with QUIC + Relay transport"
    );

    Ok(swarm)
}
