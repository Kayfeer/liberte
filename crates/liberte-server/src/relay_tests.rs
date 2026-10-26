//! Exercises the actual server relay and client swarms, not a stand-in server.
//! Disabling mDNS and DCUtR only in this fixture makes the circuit mandatory.
//! This proves relay transport locally; it does not simulate a remote NAT.

use std::{net::UdpSocket, time::Duration};

use liberte_net::{
    dial_confirmed, network_snapshot, publish_confirmed, spawn_swarm, subscribe_topic,
    swarm::SwarmConfig, SwarmCommand, SwarmNotification,
};
use liberte_shared::{
    crypto,
    identity::Identity,
    protocol::{ChatMessage, WireMessage},
    types::ChannelId,
};
use libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use tokio::sync::mpsc::{Receiver, Sender};

use super::spawn_relay;
use crate::config::ServerConfig;

fn isolated_client() -> SwarmConfig {
    SwarmConfig {
        bootstrap_peers_path: None,
        listen_port: 0,
        extra_dials: vec![],
        enable_mdns: false,
        enable_dcutr: false,
        loopback_only: true,
    }
}

async fn reserved_circuit(tx: &Sender<SwarmCommand>, relay: PeerId, own: PeerId) -> Multiaddr {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let snapshot = network_snapshot(tx).await.unwrap();
            for address in snapshot
                .listen_addresses
                .into_iter()
                .chain(snapshot.external_addresses)
            {
                let address: Multiaddr = address.parse().unwrap();
                if address.iter().any(|part| part == Protocol::P2pCircuit) {
                    let peers: Vec<_> = address
                        .iter()
                        .filter_map(|part| match part {
                            Protocol::P2p(peer) => Some(peer),
                            _ => None,
                        })
                        .collect();
                    assert_eq!(peers, vec![relay, own]);
                    return address;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Identify must reserve and advertise a real relay circuit")
}

async fn relayed_peer_subscribed(
    notifications: &mut Receiver<SwarmNotification>,
    peer: PeerId,
    topic: &str,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut connected = false;
        let mut subscribed = false;
        while let Some(event) = notifications.recv().await {
            match event {
                SwarmNotification::PeerConnected { peer_id, address } if peer_id == peer => {
                    assert!(address.iter().any(|part| part == Protocol::P2pCircuit));
                    connected = true;
                }
                SwarmNotification::PeerSubscribed {
                    peer_id,
                    topic: received,
                } if peer_id == peer && received == topic => subscribed = true,
                _ => {}
            }
            if connected && subscribed {
                return;
            }
        }
        panic!("client notification stream stopped before relay subscription");
    })
    .await
    .expect("the remote peer and channel subscription must cross the circuit");
}

async fn exchange(
    tx: &Sender<SwarmCommand>,
    received: &mut Receiver<SwarmNotification>,
    identity: &Identity,
    source: PeerId,
    channel: &ChannelId,
    key: &[u8; 32],
    plaintext: &[u8],
) {
    let message = WireMessage::ChatMessage(ChatMessage {
        sender: identity.user_id(),
        channel_id: channel.clone(),
        encrypted_content: crypto::encrypt(key, plaintext).unwrap(),
        timestamp: chrono::Utc::now(),
        message_id: uuid::Uuid::new_v4(),
    });
    let sealed = message.seal(channel.clone(), key, identity).unwrap();
    publish_confirmed(tx, channel.to_topic(), sealed)
        .await
        .unwrap();
    let data = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = received.recv().await {
            if let SwarmNotification::MessageReceived {
                source: author,
                topic,
                data,
            } = event
            {
                assert_eq!(author, Some(source));
                assert_eq!(topic, channel.to_topic());
                return data;
            }
        }
        panic!("client notification stream stopped before the message");
    })
    .await
    .expect("authenticated message must arrive through the server relay");
    let (author, message) = WireMessage::open(&data, channel.clone(), key).unwrap();
    assert_eq!(author, identity.user_id());
    let WireMessage::ChatMessage(message) = message else {
        panic!("unexpected message type");
    };
    assert_eq!(message.sender, identity.user_id());
    assert_eq!(
        crypto::decrypt(key, &message.encrypted_content).unwrap(),
        plaintext
    );
}

#[tokio::test]
async fn production_relay_reserves_a_circuit_and_carries_authenticated_messages_both_ways() {
    let directory = tempfile::tempdir().unwrap();
    let port = UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address = format!("/ip4/127.0.0.1/udp/{port}/quic-v1");
    let config = ServerConfig {
        listen_addr: address.clone(),
        public_relay_addr: Some(address.clone()),
        blob_storage_path: directory.path().to_owned(),
        ..ServerConfig::default()
    };
    let relay = spawn_relay(&config).await.unwrap();
    let relay_address: Multiaddr = format!("{address}/p2p/{relay}").parse().unwrap();
    let alice = Identity::generate();
    let bob = Identity::generate();
    let (alice_tx, mut alice_rx, alice_peer) = spawn_swarm(
        libp2p::identity::Keypair::ed25519_from_bytes(*alice.secret_bytes()).unwrap(),
        isolated_client(),
    )
    .await
    .unwrap();
    let (bob_tx, mut bob_rx, bob_peer) = spawn_swarm(
        libp2p::identity::Keypair::ed25519_from_bytes(*bob.secret_bytes()).unwrap(),
        isolated_client(),
    )
    .await
    .unwrap();
    let channel = ChannelId::new();
    let key = crypto::generate_symmetric_key();
    subscribe_topic(&alice_tx, &channel).await.unwrap();
    subscribe_topic(&bob_tx, &channel).await.unwrap();

    // Alice connects only to the relay; Identify performs the reservation.
    dial_confirmed(&alice_tx, relay_address).await.unwrap();
    let circuit = reserved_circuit(&alice_tx, relay, alice_peer).await;
    // Bob receives no direct Alice address and has no LAN discovery or DCUtR.
    dial_confirmed(&bob_tx, circuit).await.unwrap();
    relayed_peer_subscribed(&mut alice_rx, bob_peer, &channel.to_topic()).await;
    relayed_peer_subscribed(&mut bob_rx, alice_peer, &channel.to_topic()).await;
    for tx in [&alice_tx, &bob_tx] {
        assert_eq!(network_snapshot(tx).await.unwrap().connection_mode, "relayed");
    }
    exchange(
        &alice_tx,
        &mut bob_rx,
        &alice,
        alice_peer,
        &channel,
        &key,
        b"Alice through the production relay",
    )
    .await;
    exchange(
        &bob_tx,
        &mut alice_rx,
        &bob,
        bob_peer,
        &channel,
        &key,
        b"Bob through the production relay",
    )
    .await;
    for tx in [&alice_tx, &bob_tx] {
        assert_eq!(network_snapshot(tx).await.unwrap().connection_mode, "relayed");
        tx.send(SwarmCommand::Shutdown).await.unwrap();
    }
}
