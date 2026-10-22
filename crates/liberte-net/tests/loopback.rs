use std::time::Duration;

use liberte_net::{dial_confirmed, network_snapshot, publish_confirmed, spawn_swarm, subscribe_topic, SwarmCommand, SwarmNotification};
use liberte_net::swarm::SwarmConfig;
use liberte_shared::{crypto, identity::Identity, protocol::{ChatMessage, WireMessage}, types::ChannelId};

fn isolated_config() -> SwarmConfig {
    SwarmConfig { bootstrap_peers_path: None, listen_port: 0, extra_dials: vec![], enable_mdns: false, loopback_only: true }
}

async fn listen_address(tx: &tokio::sync::mpsc::Sender<SwarmCommand>) -> libp2p::Multiaddr {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(address) = network_snapshot(tx).await.unwrap().listen_addresses.into_iter().next() {
                return address.parse().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("loopback listener should start")
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
    assert!(publish_confirmed(&tx, channel.to_topic(), vec![1, 2, 3]).await.is_err());
    tx.send(SwarmCommand::Shutdown).await.unwrap();
}

#[tokio::test]
async fn two_loopback_clients_exchange_an_authenticated_encrypted_message() {
    let (alice_tx, mut alice_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let (bob_tx, mut bob_rx, bob_peer) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let channel = ChannelId::new();
    subscribe_topic(&alice_tx, &channel).await.unwrap();
    let bob_address = listen_address(&bob_tx).await;
    dial_confirmed(&alice_tx, bob_address).await.unwrap();
    // Joining after discovery/connection must notify existing channel members.
    subscribe_topic(&bob_tx, &channel).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = alice_rx.recv().await {
            if let SwarmNotification::PeerSubscribed { peer_id, topic } = notification {
                if peer_id == bob_peer && topic == channel.to_topic() { return; }
            }
        }
        panic!("subscription notification stream stopped");
    }).await.expect("late channel subscription should be observable");
    let alice_notifications = tokio::spawn(async move { while alice_rx.recv().await.is_some() {} });

    let identity = Identity::generate();
    let key = crypto::generate_symmetric_key();
    let message = WireMessage::ChatMessage(ChatMessage {
        sender: identity.user_id(), channel_id: channel.clone(), encrypted_content: crypto::encrypt(&key, b"loopback authenticated message").unwrap(),
        timestamp: chrono_now(), message_id: new_uuid(),
    });
    let data = message.seal(channel.clone(), &key, &identity).unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if publish_confirmed(&alice_tx, channel.to_topic(), data.clone()).await.is_ok() { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("remote subscription must propagate");

    let received = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(notification) = bob_rx.recv().await {
            if let SwarmNotification::MessageReceived { data, .. } = notification { return data; }
        }
        panic!("receiver unexpectedly stopped");
    }).await.expect("Bob should receive the message");
    let (author, decoded) = WireMessage::open(&received, channel, &key).unwrap();
    assert_eq!(author, identity.user_id());
    if let WireMessage::ChatMessage(chat) = decoded {
        assert_eq!(crypto::decrypt(&key, &chat.encrypted_content).unwrap(), b"loopback authenticated message");
    } else { panic!("wrong message type"); }
    alice_tx.send(SwarmCommand::Shutdown).await.unwrap();
    bob_tx.send(SwarmCommand::Shutdown).await.unwrap();
    alice_notifications.await.unwrap();
}

fn chrono_now() -> chrono::DateTime<chrono::Utc> { chrono::Utc::now() }
fn new_uuid() -> uuid::Uuid { uuid::Uuid::new_v4() }
