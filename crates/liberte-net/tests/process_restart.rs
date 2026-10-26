//! Exercise a real process crash: dropping a swarm normally sends QUIC close
//! frames and cannot reproduce a surviving peer's stale GossipSub connection.
use std::{io::Write as _, process::Stdio, time::Duration};

use liberte_net::{
    network_snapshot, publish_confirmed, spawn_swarm, subscribe_topic,
    swarm::SwarmConfig, SwarmCommand, SwarmNotification,
};
use liberte_shared::{
    crypto,
    identity::Identity,
    protocol::{ChatMessage, WireMessage},
    types::{ChannelId, UserId},
};
use libp2p::{identity::Keypair, multiaddr::Protocol, Multiaddr};
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
    sync::mpsc,
};

const PHASE_ENV: &str = "LIBERTE_PROCESS_RESTART_TEST_PHASE";
const CHANNEL_ENV: &str = "LIBERTE_PROCESS_RESTART_TEST_CHANNEL";
const ADDRESS_ENV: &str = "LIBERTE_PROCESS_RESTART_TEST_ALICE_ADDRESS";
const ALICE_ID_ENV: &str = "LIBERTE_PROCESS_RESTART_TEST_ALICE_ID";
const BOB_SEED: [u8; 32] = [0x42; 32];
const CHANNEL_KEY: [u8; 32] = [0x73; 32];
const DEADLINE: Duration = Duration::from_secs(20);

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

async fn listen_address(tx: &mpsc::Sender<SwarmCommand>) -> Multiaddr {
    tokio::time::timeout(DEADLINE, async {
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
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("QUIC listener should acquire its actual port")
}

fn seal(identity: &Identity, channel: &ChannelId, text: &str) -> Vec<u8> {
    WireMessage::ChatMessage(ChatMessage {
        sender: identity.user_id(),
        channel_id: channel.clone(),
        encrypted_content: crypto::encrypt(&CHANNEL_KEY, text.as_bytes()).unwrap(),
        timestamp: chrono::Utc::now(),
        message_id: uuid::Uuid::new_v4(),
    })
    .seal(channel.clone(), &CHANNEL_KEY, identity)
    .unwrap()
}

fn message_matches(data: &[u8], channel: &ChannelId, author: &UserId, expected: &str) -> bool {
    let (actual_author, message) =
        WireMessage::open(data, channel.clone(), &CHANNEL_KEY).unwrap();
    assert_eq!(&actual_author, author);
    let WireMessage::ChatMessage(message) = message else {
        panic!("Expected an authenticated chat message");
    };
    assert_eq!(message.channel_id, *channel);
    crypto::decrypt(&CHANNEL_KEY, &message.encrypted_content).unwrap() == expected.as_bytes()
}

struct ChildPeer {
    child: Child,
    output: Lines<BufReader<ChildStdout>>,
}

fn launch_bob(phase: &str, channel: &ChannelId, address: &Multiaddr, alice: &Identity) -> ChildPeer {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "process_restart_child", "--nocapture"])
        .env(PHASE_ENV, phase)
        .env(CHANNEL_ENV, channel.0.to_string())
        .env(ADDRESS_ENV, address.to_string())
        .env(ALICE_ID_ENV, alice.user_id().to_hex())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("The integration test binary should launch its isolated peer");
    let output = BufReader::new(child.stdout.take().unwrap()).lines();
    ChildPeer { child, output }
}

async fn child_marker(child: &mut ChildPeer, prefix: &str) -> String {
    tokio::time::timeout(DEADLINE, async {
        while let Some(line) = child.output.next_line().await.unwrap() {
            if let Some((_, value)) = line.split_once(prefix) {
                return value.to_owned();
            }
        }
        panic!("Peer process ended before reporting {prefix}");
    })
    .await
    .expect("Peer process should report its actual network progress")
}

async fn receive_from_bob(
    rx: &mut mpsc::Receiver<SwarmNotification>,
    child: &mut ChildPeer,
    channel: &ChannelId,
    expected: &str,
) -> bool {
    let bob = Identity::from_secret_bytes(&BOB_SEED);
    let bob_peer = Keypair::ed25519_from_bytes(BOB_SEED)
        .unwrap()
        .public()
        .to_peer_id();
    tokio::time::timeout(DEADLINE, async {
        let mut disconnected = false;
        loop {
            tokio::select! {
                notification = rx.recv() => match notification {
                    Some(SwarmNotification::PeerDisconnected { peer_id }) if peer_id == bob_peer => {
                        disconnected = true;
                    }
                    Some(SwarmNotification::MessageReceived { topic, data, .. }) => {
                        assert_eq!(topic, channel.to_topic());
                        if message_matches(&data, channel, &bob.user_id(), expected) {
                            return disconnected;
                        }
                    }
                    Some(_) => {},
                    None => panic!("Alice's swarm stopped before receiving {expected}"),
                },
                status = child.child.wait() => {
                    panic!("Bob exited before publishing {expected}: {:?}", status.unwrap());
                },
            }
        }
    })
    .await
    .expect("Bob's authenticated message should arrive after connecting")
}

/// This test entry point is also the subprocess executable. In the ordinary test
/// run it does nothing; only launch_bob supplies its isolated test environment.
#[test]
fn process_restart_child() {
    let Ok(phase) = std::env::var(PHASE_ENV) else {
        return;
    };
    assert!(matches!(phase.as_str(), "before-crash" | "after-crash"));
    let channel = ChannelId(
        uuid::Uuid::parse_str(&std::env::var(CHANNEL_ENV).unwrap()).unwrap(),
    );
    let address: Multiaddr = std::env::var(ADDRESS_ENV).unwrap().parse().unwrap();
    let alice_id = UserId::from_hex(&std::env::var(ALICE_ID_ENV).unwrap()).unwrap();
    let alice_peer = liberte_net::swarm::target_peer_id(&address).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(DEADLINE, async {
            let identity = Identity::from_secret_bytes(&BOB_SEED);
            let mut config = isolated_config();
            // These are the same explicit connection hints restored by a client
            // profile. The OS chooses a fresh listener port for the new process.
            config.extra_dials = vec![address];
            let (tx, mut rx, _) =
                spawn_swarm(Keypair::ed25519_from_bytes(BOB_SEED).unwrap(), config)
                    .await
                    .unwrap();
            subscribe_topic(&tx, &channel).await.unwrap();
            let own_address = listen_address(&tx).await;
            println!("LIBERTE_RESTART_BOOT {own_address}");
            std::io::stdout().flush().unwrap();
            loop {
                match rx.recv().await {
                    Some(SwarmNotification::PeerConnected { peer_id, .. })
                        if peer_id == alice_peer => break,
                    Some(_) => {},
                    None => panic!("Bob's swarm stopped before its connection"),
                }
            }
            // Match the product path: submit once after a transport connection.
            // publish_confirmed may wait for subscription propagation itself.
            publish_confirmed(&tx, channel.to_topic(), seal(&identity, &channel, &phase))
                .await
                .expect("The restarted peer must receive the surviving peer's subscriptions");
            let acknowledgement = format!("ack-{phase}");
            loop {
                match rx.recv().await {
                    Some(SwarmNotification::MessageReceived { topic, data, .. }) => {
                        assert_eq!(topic, channel.to_topic());
                        if message_matches(&data, &channel, &alice_id, &acknowledgement) {
                            break;
                        }
                    }
                    Some(_) => {},
                    None => panic!("Bob's swarm stopped before receiving Alice's reply"),
                }
            }
            println!("LIBERTE_RESTART_ACK {phase}");
            std::io::stdout().flush().unwrap();
            if phase == "before-crash" {
                // Do not shut down or drop this swarm gracefully. The parent
                // forcibly kills this OS process with its QUIC sessions active.
                while rx.recv().await.is_some() {}
                panic!("The first process must remain alive until killed");
            }
            tx.send(SwarmCommand::Shutdown).await.unwrap();
        })
        .await
        .expect("Child peer lifecycle should finish within its deadline");
    });
}

#[tokio::test]
async fn authenticated_chat_survives_an_abrupt_peer_process_restart() {
    let alice = Identity::generate();
    let channel = ChannelId::new();
    let (tx, mut rx, _) = spawn_swarm(
        Keypair::ed25519_from_bytes(*alice.secret_bytes()).unwrap(),
        isolated_config(),
    )
    .await
    .unwrap();
    subscribe_topic(&tx, &channel).await.unwrap();
    let address = listen_address(&tx).await;
    let bob_peer = Keypair::ed25519_from_bytes(BOB_SEED)
        .unwrap()
        .public()
        .to_peer_id();

    let mut first = launch_bob("before-crash", &channel, &address, &alice);
    let previous_address: Multiaddr = child_marker(&mut first, "LIBERTE_RESTART_BOOT ")
        .await
        .parse()
        .unwrap();
    receive_from_bob(&mut rx, &mut first, &channel, "before-crash").await;
    publish_confirmed(
        &tx,
        channel.to_topic(),
        seal(&alice, &channel, "ack-before-crash"),
    )
    .await
    .unwrap();
    assert_eq!(
        child_marker(&mut first, "LIBERTE_RESTART_ACK ").await,
        "before-crash"
    );
    first.child.kill().await.unwrap();

    // An OS process kill sends no QUIC close frames. Assert the regression's
    // precondition instead of waiting for the survivor's idle timeout.
    assert!(network_snapshot(&tx)
        .await
        .unwrap()
        .peers
        .contains(&bob_peer.to_string()));
    let previous_port = previous_address
        .iter()
        .find_map(|protocol| match protocol {
            Protocol::Udp(port) => Some(port),
            _ => None,
        })
        .unwrap();
    // Keep the old listener port unavailable so the child cannot accidentally
    // receive the same ephemeral port. This socket does not emulate a peer.
    let old_port = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, previous_port))
        .expect("An exited process must release its UDP listener");
    let mut restarted = launch_bob("after-crash", &channel, &address, &alice);
    let restarted_address: Multiaddr = child_marker(&mut restarted, "LIBERTE_RESTART_BOOT ")
        .await
        .parse()
        .unwrap();
    assert_ne!(restarted_address, previous_address);
    assert_eq!(
        liberte_net::swarm::target_peer_id(&restarted_address),
        Some(bob_peer)
    );
    assert!(
        receive_from_bob(&mut rx, &mut restarted, &channel, "after-crash").await,
        "Alice must discard the previous peer session before chat resumes"
    );
    publish_confirmed(
        &tx,
        channel.to_topic(),
        seal(&alice, &channel, "ack-after-crash"),
    )
    .await
    .unwrap();
    assert_eq!(
        child_marker(&mut restarted, "LIBERTE_RESTART_ACK ").await,
        "after-crash"
    );
    // Keep old packets silently undeliverable through the full exchange. An
    // unbound port could emit ICMP and clear stale QUIC state by accident.
    drop(old_port);
    assert!(tokio::time::timeout(DEADLINE, restarted.child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    tx.send(SwarmCommand::Shutdown).await.unwrap();
}
