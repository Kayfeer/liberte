//! Exercises the real QUIC/GossipSub boundary without starting Tauri or touching user files.
//! The desktop transfer state machine has separate tests in liberte-client/file_transfer.rs.
use std::{collections::HashSet, time::Duration};

use liberte_net::{dial_confirmed, network_snapshot, publish_confirmed, spawn_swarm, subscribe_topic, SwarmCommand, SwarmNotification};
use liberte_net::swarm::SwarmConfig;
use liberte_shared::{crypto, identity::Identity, protocol::{FileAcceptance, FileChunk, FileCompletion, FileOffer, ReplayGuard, WireMessage}, types::ChannelId};
use tokio::sync::mpsc;

const CHUNK_SIZE: usize = 32 * 1024;

fn isolated_config() -> SwarmConfig {
    SwarmConfig { bootstrap_peers_path: None, listen_port: 0, extra_dials: vec![], enable_mdns: false, loopback_only: true }
}

async fn first_address(tx: &mpsc::Sender<SwarmCommand>) -> libp2p::Multiaddr {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(address) = network_snapshot(tx).await.unwrap().listen_addresses.into_iter().next() {
                return address.parse().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("loopback listener should start")
}

async fn publish_when_ready(tx: &mpsc::Sender<SwarmCommand>, channel: &ChannelId, bytes: Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if publish_confirmed(tx, channel.to_topic(), bytes.clone()).await.is_ok() { return; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("remote subscription should propagate")
}

async fn receive(rx: &mut mpsc::Receiver<SwarmNotification>, channel: &ChannelId) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(8), async {
        while let Some(notification) = rx.recv().await {
            if let SwarmNotification::MessageReceived { topic, data, .. } = notification {
                if topic == channel.to_topic() { return data; }
            }
        }
        panic!("network notification stream stopped");
    }).await.expect("peer should receive published message")
}

#[tokio::test]
async fn authenticated_file_offer_accept_chunks_and_receipt_cross_real_quic() {
    let (alice_tx, mut alice_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let (bob_tx, mut bob_rx, _) = spawn_swarm(libp2p::identity::Keypair::generate_ed25519(), isolated_config()).await.unwrap();
    let alice = Identity::generate();
    let bob = Identity::generate();
    let channel = ChannelId::new();
    let channel_key = crypto::generate_symmetric_key();
    subscribe_topic(&alice_tx, &channel).await.unwrap();
    subscribe_topic(&bob_tx, &channel).await.unwrap();
    dial_confirmed(&alice_tx, first_address(&bob_tx).await).await.unwrap();

    let source: Vec<u8> = (0..CHUNK_SIZE * 3 + 17).map(|index| (index % 251) as u8).collect();
    let expected_hash = *blake3::hash(&source).as_bytes();
    let file_id = uuid::Uuid::new_v4();
    let offer = WireMessage::FileOffer(FileOffer {
        sender: alice.user_id(), channel_id: channel.clone(), file_id,
        file_name: "loopback.bin".into(), file_size: source.len() as u64,
        file_hash: expected_hash, timestamp: chrono::Utc::now(),
    });
    publish_when_ready(&alice_tx, &channel, offer.seal(channel.clone(), &channel_key, &alice).unwrap()).await;
    let offered_bytes = receive(&mut bob_rx, &channel).await;
    let (offer_author, received_offer) = WireMessage::open(&offered_bytes, channel.clone(), &channel_key).unwrap();
    assert_eq!(offer_author, alice.user_id());
    let WireMessage::FileOffer(received_offer) = received_offer else { panic!("expected file offer"); };
    assert_eq!(received_offer.file_id, file_id);
    assert_eq!(received_offer.file_size, source.len() as u64);
    assert_eq!(received_offer.file_hash, expected_hash);

    let acceptance = WireMessage::FileAccept(FileAcceptance { file_id, accepter: bob.user_id() });
    // Sharing the channel key does not allow Bob to announce an acceptance signed as Alice.
    let forged_acceptance = WireMessage::FileAccept(FileAcceptance { file_id, accepter: alice.user_id() });
    assert!(forged_acceptance.seal(channel.clone(), &channel_key, &bob).is_err());
    publish_when_ready(&bob_tx, &channel, acceptance.seal(channel.clone(), &channel_key, &bob).unwrap()).await;
    let (accepted_by, accepted) = WireMessage::open(&receive(&mut alice_rx, &channel).await, channel.clone(), &channel_key).unwrap();
    assert_eq!(accepted_by, bob.user_id());
    let WireMessage::FileAccept(accepted) = accepted else { panic!("expected acceptance"); };
    assert_eq!(accepted.accepter, accepted_by);
    assert_eq!(accepted.file_id, file_id);

    // A corrupted packet crosses the transport but fails authenticated application decoding.
    let probe = WireMessage::FileChunk(FileChunk { file_id, chunk_index: 0, total_chunks: 4, data: source[..CHUNK_SIZE].to_vec() });
    let mut corrupted = probe.seal(channel.clone(), &channel_key, &alice).unwrap();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    publish_when_ready(&alice_tx, &channel, corrupted).await;
    assert!(WireMessage::open(&receive(&mut bob_rx, &channel).await, channel.clone(), &channel_key).is_err());
    publish_when_ready(&alice_tx, &channel, probe.to_bytes().unwrap()).await;
    assert!(WireMessage::open(&receive(&mut bob_rx, &channel).await, channel.clone(), &channel_key).is_err());

    // Four chunks, including a short tail, arrive out of order with one retransmission.
    let mut assembled = vec![0; received_offer.file_size as usize];
    let mut seen_chunks = HashSet::new();
    let mut replay_guard = ReplayGuard::default();
    for index in [3u32, 1, 1, 0, 2] {
        let offset = index as usize * CHUNK_SIZE;
        let end = (offset + CHUNK_SIZE).min(source.len());
        let packet = WireMessage::FileChunk(FileChunk { file_id, chunk_index: index, total_chunks: 4, data: source[offset..end].to_vec() });
        publish_when_ready(&alice_tx, &channel, packet.seal(channel.clone(), &channel_key, &alice).unwrap()).await;
        let bytes = receive(&mut bob_rx, &channel).await;
        let verified = WireMessage::open_authenticated(&bytes, channel.clone(), &channel_key).unwrap();
        assert_eq!(verified.author, offer_author);
        assert!(replay_guard.accept(&verified));
        let WireMessage::FileChunk(chunk) = verified.message else { panic!("expected chunk"); };
        assert_eq!(chunk.file_id, file_id);
        assert_eq!(chunk.total_chunks, 4);
        let offset = chunk.chunk_index as usize * CHUNK_SIZE;
        let end = offset + chunk.data.len();
        assert!(end <= assembled.len());
        if seen_chunks.insert(chunk.chunk_index) { assembled[offset..end].copy_from_slice(&chunk.data); }
        assert!(seen_chunks.len() <= 4);
    }
    assert_eq!(seen_chunks.len(), 4);
    assert_eq!(assembled, source);
    assert_eq!(*blake3::hash(&assembled).as_bytes(), expected_hash);

    let receipt = WireMessage::FileComplete(FileCompletion { file_id, receiver: bob.user_id(), channel_id: channel.clone(), file_hash: expected_hash });
    publish_when_ready(&bob_tx, &channel, receipt.seal(channel.clone(), &channel_key, &bob).unwrap()).await;
    let (receipt_author, receipt) = WireMessage::open(&receive(&mut alice_rx, &channel).await, channel.clone(), &channel_key).unwrap();
    let WireMessage::FileComplete(receipt) = receipt else { panic!("expected completion receipt"); };
    assert_eq!(receipt_author, accepted_by);
    assert_eq!(receipt.receiver, receipt_author);
    assert_eq!(receipt.file_id, file_id);
    assert_eq!(receipt.file_hash, expected_hash);
    assert_eq!(receipt.channel_id, channel);
    alice_tx.send(SwarmCommand::Shutdown).await.unwrap();
    bob_tx.send(SwarmCommand::Shutdown).await.unwrap();
}
