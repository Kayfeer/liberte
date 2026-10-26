use std::{collections::HashSet, time::Duration};

use liberte_net::swarm::SwarmConfig;
use liberte_net::{
    dial_addresses_confirmed, network_snapshot, publish_confirmed, publish_once, spawn_swarm, subscribe_topic,
    SwarmCommand, SwarmNotification,
};
use liberte_shared::{
    crypto,
    identity::Identity,
    protocol::{ChatMessage, WireMessage},
    types::ChannelId,
};

fn isolated_config() -> SwarmConfig {
    SwarmConfig {
        bootstrap_peers_path: None,
        listen_port: 0,
        extra_dials: vec![],
        enable_mdns: false,
        enable_dcutr: true,
        loopback_only: true,
    }
}

async fn listen_address(tx: &tokio::sync::mpsc::Sender<SwarmCommand>) -> libp2p::Multiaddr {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(address) = network_snapshot(tx)
                .await
                .unwrap()
                .listen_addresses
                .into_iter()
                .next()
            {
                return address.parse().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("loopback listener should start")
}

#[test]
fn libp2p_identity_constructor_accepts_seed_not_expanded_keypair() {
    assert!(libp2p::identity::Keypair::ed25519_from_bytes([7u8; 32]).is_ok());
    assert!(libp2p::identity::Keypair::ed25519_from_bytes([7u8; 64]).is_err());
}

#[tokio::test]
async fn publication_without_subscribers_returns_an_error() {
    let key = libp2p::identity::Keypair::generate_ed25519();
    let (tx, _rx, _) = spawn_swarm(key, isolated_config()).await.unwrap();
    let channel = ChannelId::new();
    subscribe_topic(&tx, &channel).await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(1), publish_once(&tx, channel.to_topic(), vec![4, 5, 6]))
        .await.expect("background announcements must not wait for subscribers").is_err());
    assert!(publish_confirmed(&tx, channel.to_topic(), vec![1, 2, 3])
        .await
        .is_err());
    tx.send(SwarmCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn two_loopback_clients_exchange_an_authenticated_encrypted_message() {
    let (alice_tx, mut alice_rx, _) = spawn_swarm(
        libp2p::identity::Keypair::generate_ed25519(),
        isolated_config(),
    )
    .await
    .unwrap();
    let (bob_tx, mut bob_rx, bob_peer) = spawn_swarm(
        libp2p::identity::Keypair::generate_ed25519(),
        isolated_config(),
    )
    .await
    .unwrap();
    let channel = ChannelId::new();
    let mut expected_topics = HashSet::from([channel.to_topic()]);
    // A reconnect announces all subscriptions in one RPC. Cover both the new
    // upstream 100-topic request limit and its 100-topic cumulative limit.
    for _ in 0..127 {
        let existing = ChannelId::new();
        expected_topics.insert(existing.to_topic());
        subscribe_topic(&alice_tx, &existing).await.unwrap();
        subscribe_topic(&bob_tx, &existing).await.unwrap();
    }
    subscribe_topic(&alice_tx, &channel).await.unwrap();
    let bob_address = listen_address(&bob_tx).await;
    // An unsupported first address must not prevent the reachable QUIC alternative.
    let unavailable: libp2p::Multiaddr = format!("/ip4/127.0.0.1/tcp/1/p2p/{bob_peer}").parse().unwrap();
    dial_addresses_confirmed(&alice_tx, vec![unavailable, bob_address]).await.unwrap();
    // Joining after discovery/connection must notify existing channel members.
    subscribe_topic(&bob_tx, &channel).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = alice_rx.recv().await {
            if let SwarmNotification::PeerSubscribed { peer_id, topic } = notification {
                if peer_id == bob_peer {
                    expected_topics.remove(&topic);
                    if expected_topics.is_empty() {
                        return;
                    }
                }
            }
        }
        panic!("subscription notification stream stopped");
    })
    .await
    .expect("existing and late channel subscriptions should all be observable");
    let alice_notifications = tokio::spawn(async move { while alice_rx.recv().await.is_some() {} });

    let identity = Identity::generate();
    let key = crypto::generate_symmetric_key();
    let message = WireMessage::ChatMessage(ChatMessage {
        sender: identity.user_id(),
        channel_id: channel.clone(),
        encrypted_content: crypto::encrypt(&key, b"loopback authenticated message").unwrap(),
        timestamp: chrono_now(),
        message_id: new_uuid(),
    });
    let data = message.seal(channel.clone(), &key, &identity).unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if publish_confirmed(&alice_tx, channel.to_topic(), data.clone())
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("remote subscription must propagate");

    let received = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = bob_rx.recv().await {
            if let SwarmNotification::MessageReceived { data, .. } = notification {
                return data;
            }
        }
        panic!("receiver unexpectedly stopped");
    })
    .await
    .expect("Bob should receive the message");
    let (author, decoded) = WireMessage::open(&received, channel, &key).unwrap();
    assert_eq!(author, identity.user_id());
    if let WireMessage::ChatMessage(chat) = decoded {
        assert_eq!(
            crypto::decrypt(&key, &chat.encrypted_content).unwrap(),
            b"loopback authenticated message"
        );
    } else {
        panic!("wrong message type");
    }
    alice_tx.send(SwarmCommand::Shutdown).await.unwrap();
    bob_tx.send(SwarmCommand::Shutdown).await.unwrap();
    alice_notifications.await.unwrap();
}

fn chrono_now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
fn new_uuid() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}

#[tokio::test]
async fn occupied_default_port_uses_an_available_quic_listener() {
    let occupied = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, liberte_shared::constants::DEFAULT_QUIC_PORT)).unwrap();
    let mut config = isolated_config();
    config.listen_port = liberte_shared::constants::DEFAULT_QUIC_PORT;
    let (tx, _rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), config).await.unwrap();
    let address = listen_address(&tx).await;
    let port = address.iter().find_map(|protocol| match protocol {
        libp2p::multiaddr::Protocol::Udp(port) => Some(port), _ => None,
    }).unwrap();
    assert_ne!(port, liberte_shared::constants::DEFAULT_QUIC_PORT);
    assert_ne!(port, 0);
    tx.send(SwarmCommand::Shutdown).await.unwrap();
    drop(occupied);
}

#[tokio::test]
async fn explicit_peer_reconnects_after_the_remote_process_restarts() {
    let remote_key = libp2p::identity::Keypair::generate_ed25519();
    let (alice_tx, mut alice_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let (bob_tx, mut bob_rx, bob_peer) = spawn_swarm(remote_key.clone(), isolated_config()).await.unwrap();
    let bob_notifications = tokio::spawn(async move { while bob_rx.recv().await.is_some() {} });
    let address = listen_address(&bob_tx).await;
    let port = address.iter().find_map(|p| match p { libp2p::multiaddr::Protocol::Udp(port) => Some(port), _ => None }).unwrap();
    dial_addresses_confirmed(&alice_tx, vec![address]).await.unwrap();
    bob_tx.send(SwarmCommand::Shutdown).await.unwrap();
    bob_notifications.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = alice_rx.recv().await {
            if matches!(notification, SwarmNotification::PeerDisconnected { peer_id } if peer_id == bob_peer) { return; }
        }
        panic!("local swarm stopped");
    }).await.expect("remote shutdown should be observed");
    // In one runtime, Quinn's close/draining tasks can briefly outlive the
    // swarm itself. A real process exit releases its sockets; wait for that
    // same condition without changing the cached destination port.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, port)) {
                Ok(socket) => { drop(socket); break; },
                Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                },
                Err(error) => panic!("Cannot probe released QUIC port: {error}"),
            }
        }
    }).await.expect("shutdown should release the QUIC listener port");
    let mut config = isolated_config();
    config.listen_port = port;
    let (restarted_tx, mut restarted_rx, restarted_peer) = spawn_swarm(remote_key, config).await.unwrap();
    assert_eq!(restarted_peer, bob_peer);
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(notification) = alice_rx.recv().await {
            if matches!(notification, SwarmNotification::PeerConnected { peer_id, .. } if peer_id == bob_peer) { return; }
        }
        panic!("local swarm stopped");
    }).await.expect("explicit peer should reconnect without mDNS or another user dial");
    let remote_notifications = tokio::spawn(async move { while restarted_rx.recv().await.is_some() {} });
    alice_tx.send(SwarmCommand::Shutdown).await.unwrap();
    restarted_tx.send(SwarmCommand::Shutdown).await.unwrap();
    remote_notifications.await.unwrap();
}

#[tokio::test]
async fn first_publication_waits_for_the_remote_subscription() {
    let (alice_tx, mut alice_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let (bob_tx, mut bob_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let alice_notifications = tokio::spawn(async move { while alice_rx.recv().await.is_some() {} });
    let channel = ChannelId::new();
    subscribe_topic(&alice_tx, &channel).await.unwrap();
    dial_addresses_confirmed(&alice_tx, vec![listen_address(&bob_tx).await]).await.unwrap();
    let delayed_tx = bob_tx.clone();
    let delayed_channel = channel.clone();
    let delayed_subscription = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        subscribe_topic(&delayed_tx, &delayed_channel).await.unwrap();
    });
    publish_confirmed(&alice_tx, channel.to_topic(), b"first message".to_vec()).await.unwrap();
    delayed_subscription.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = bob_rx.recv().await {
            if let SwarmNotification::MessageReceived { data, .. } = notification {
                assert_eq!(data, b"first message");
                return;
            }
        }
        panic!("remote swarm stopped");
    }).await.expect("first publication should arrive after the subscription handshake");
    alice_tx.send(SwarmCommand::Shutdown).await.unwrap();
    bob_tx.send(SwarmCommand::Shutdown).await.unwrap();
    alice_notifications.await.unwrap();
}
