use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use liberte_net::{SwarmCommand, SwarmNotification};
use liberte_shared::crypto;
use liberte_shared::protocol::WireMessage;
use liberte_shared::types::ChannelId;
use liberte_store::Message;

use crate::events::*;
use crate::state::AppState;

/// Start the libp2p swarm, store `cmd_tx` in AppState, and spawn the
/// notification processing loop that forwards events to the Tauri frontend.
pub async fn start_swarm_and_bridge(
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
    identity_secret: [u8; 32],
) -> Result<(), String> {
    // Derive a libp2p Ed25519 keypair from the identity secret via BLAKE3 KDF
    // (libp2p uses a different Ed25519 implementation, so we derive a separate key)
    let kdf_key = blake3::derive_key("liberte-libp2p-keypair-v1", &identity_secret);
    let libp2p_keypair = libp2p::identity::Keypair::ed25519_from_bytes(kdf_key)
        .map_err(|e| format!("Failed to create libp2p keypair: {e}"))?;

    let config = liberte_net::swarm::SwarmConfig::default();

    let (cmd_tx, notif_rx, local_peer_id) = liberte_net::spawn_swarm(libp2p_keypair, config)
        .await
        .map_err(|e| format!("Failed to spawn swarm: {e}"))?;

    info!(peer_id = %local_peer_id, "Swarm started");

    // Store cmd_tx in AppState
    {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard.swarm_cmd_tx = Some(cmd_tx.clone());
    }

    // Subscribe to all existing channels
    subscribe_all_channels(&state, &cmd_tx).await;

    // Spawn notification processing loop
    let state_clone = state.clone();
    tokio::spawn(async move {
        notification_loop(app, state_clone, notif_rx).await;
    });

    Ok(())
}

/// Subscribe to gossipsub topics for all channels the user has keys for.
async fn subscribe_all_channels(state: &Arc<Mutex<AppState>>, cmd_tx: &mpsc::Sender<SwarmCommand>) {
    let channel_keys: HashMap<String, String> = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let db = match guard.database.as_ref() {
            Some(db) => db,
            None => return,
        };
        db.get_all_channel_keys()
            .unwrap_or_default()
            .into_iter()
            .map(|(id, k)| (id.to_string(), k))
            .collect()
    };

    for channel_id_str in channel_keys.keys() {
        if let Ok(uuid) = uuid::Uuid::parse_str(channel_id_str) {
            let topic = ChannelId(uuid).to_topic();
            debug!(topic = %topic, "Auto-subscribing to channel");
            let _ = cmd_tx.send(SwarmCommand::SubscribeTopic(topic)).await;
        }
    }

    info!(
        count = channel_keys.len(),
        "Subscribed to existing channels"
    );
}

/// Main loop that receives swarm notifications and dispatches them to the
/// Tauri frontend via events, and stores incoming messages in the database.
async fn notification_loop(
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
    mut notif_rx: mpsc::Receiver<SwarmNotification>,
) {
    // Track which channels each peer is associated with (for presence)
    let mut _channel_peers: HashMap<String, HashSet<String>> = HashMap::new();

    info!("Swarm notification bridge started");

    while let Some(notification) = notif_rx.recv().await {
        match notification {
            SwarmNotification::PeerConnected { peer_id, address } => {
                info!(peer = %peer_id, addr = %address, "Peer connected (bridge)");
                emit_event(
                    &app,
                    EVENT_PEER_CONNECTED,
                    PeerEventPayload {
                        peer_id: peer_id.to_string(),
                    },
                );
            }

            SwarmNotification::PeerDisconnected { peer_id } => {
                info!(peer = %peer_id, "Peer disconnected (bridge)");
                emit_event(
                    &app,
                    EVENT_PEER_DISCONNECTED,
                    PeerEventPayload {
                        peer_id: peer_id.to_string(),
                    },
                );
            }

            SwarmNotification::MessageReceived {
                source,
                topic,
                data,
            } => {
                debug!(
                    topic = %topic,
                    source = ?source,
                    len = data.len(),
                    "Message received on bridge"
                );
                handle_incoming_message(&app, &state, &topic, &data);
            }

            SwarmNotification::RelayReservation {
                relay_peer,
                relay_addr,
            } => {
                info!(
                    relay = %relay_peer,
                    addr = %relay_addr,
                    "Relay reservation received"
                );
            }
        }
    }

    warn!("Swarm notification loop ended");
}

/// Try to decrypt and store an incoming gossipsub message.
fn handle_incoming_message(
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    topic: &str,
    data: &[u8],
) {
    // The topic format is "channel:<uuid>"
    let channel_uuid_str = match topic.strip_prefix("channel:") {
        Some(s) => s,
        None => {
            debug!(topic = %topic, "Ignoring message on non-channel topic");
            return;
        }
    };

    let channel_uuid = match uuid::Uuid::parse_str(channel_uuid_str) {
        Ok(u) => u,
        Err(e) => {
            warn!(topic = %topic, error = %e, "Invalid channel UUID in topic");
            return;
        }
    };

    // Look up the channel key
    let (channel_key, own_pubkey) = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let db = match guard.database.as_ref() {
            Some(db) => db,
            None => return,
        };

        let key_hex = match db.get_channel_key(channel_uuid) {
            Ok(k) => k,
            Err(_) => {
                debug!(channel = %channel_uuid, "No key for channel, skipping");
                return;
            }
        };

        let key_bytes = match hex::decode(&key_hex) {
            Ok(b) if b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                arr
            }
            _ => return,
        };

        let own_pk = guard.identity.as_ref().map(|id| id.public_key_bytes());

        (key_bytes, own_pk)
    };

    // The data on gossipsub is a raw WireMessage (not encrypted at transport level,
    // but the content inside ChatMessage is encrypted with the channel key).
    let wire_msg = match WireMessage::from_bytes(data) {
        Ok(m) => m,
        Err(e) => {
            // The message might be encrypted at the channel level — try decrypting first
            match crypto::decrypt(&channel_key, data) {
                Ok(plaintext) => match WireMessage::from_bytes(&plaintext) {
                    Ok(m) => m,
                    Err(e2) => {
                        debug!(error = %e, error2 = %e2, "Failed to deserialize wire message");
                        return;
                    }
                },
                Err(_) => {
                    debug!(error = %e, "Failed to deserialize wire message");
                    return;
                }
            }
        }
    };

    match wire_msg {
        WireMessage::ChatMessage(chat) => {
            // Skip our own messages (already stored locally)
            if let Some(own_pk) = own_pubkey {
                if chat.sender.0 == own_pk {
                    return;
                }
            }

            let msg = Message {
                id: chat.message_id,
                channel_id: channel_uuid,
                sender_pubkey: chat.sender.0,
                encrypted_content: chat.encrypted_content,
                timestamp: chat.timestamp,
            };

            // Store in database
            {
                let guard = match state.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(ref db) = guard.database {
                    if let Err(e) = db.insert_message(&msg) {
                        // Duplicate message IDs are expected (ignore)
                        debug!(error = %e, "Failed to store incoming message (may be duplicate)");
                    }
                }
            }

            info!(
                msg_id = %chat.message_id,
                channel = %channel_uuid,
                sender = %hex::encode(chat.sender.0)[..8],
                "Received and stored message from peer"
            );

            emit_event(
                app,
                EVENT_NEW_MESSAGE,
                NewMessagePayload {
                    channel_id: channel_uuid.to_string(),
                    sender: hex::encode(chat.sender.0),
                    message_id: chat.message_id.to_string(),
                    timestamp: chat.timestamp.to_rfc3339(),
                },
            );
        }

        WireMessage::TypingIndicator(typing) => {
            emit_event(
                app,
                EVENT_TYPING_INDICATOR,
                TypingPayload {
                    channel_id: channel_uuid.to_string(),
                    user_id: typing.sender.to_hex(),
                    display_name: typing.sender_display_name,
                },
            );
        }

        WireMessage::StatusUpdate(status) => {
            emit_event(
                app,
                EVENT_STATUS_CHANGED,
                StatusChangedPayload {
                    user_id: status.user_id.to_hex(),
                    status: status.status,
                },
            );
        }

        WireMessage::MessageReaction(reaction) => {
            let action_str = match reaction.action {
                liberte_shared::protocol::ReactionAction::Add => "add",
                liberte_shared::protocol::ReactionAction::Remove => "remove",
            };

            // Store reaction in database
            {
                let guard = match state.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                if let Some(ref db) = guard.database {
                    match reaction.action {
                        liberte_shared::protocol::ReactionAction::Add => {
                            let _ = db.add_reaction(
                                reaction.message_id,
                                channel_uuid,
                                &reaction.sender.to_hex(),
                                &reaction.emoji,
                            );
                        }
                        liberte_shared::protocol::ReactionAction::Remove => {
                            let _ = db.remove_reaction(
                                reaction.message_id,
                                &reaction.sender.to_hex(),
                                &reaction.emoji,
                            );
                        }
                    }
                }
            }

            emit_event(
                app,
                EVENT_MESSAGE_REACTION,
                ReactionPayload {
                    channel_id: channel_uuid.to_string(),
                    message_id: reaction.message_id.to_string(),
                    user_id: reaction.sender.to_hex(),
                    emoji: reaction.emoji,
                    action: action_str.to_string(),
                },
            );
        }

        WireMessage::PeerStatus(status) => {
            debug!(
                user = %status.user_id.to_hex(),
                online = status.online,
                "Received peer status"
            );
            // Could be used for presence tracking in the future
        }

        other => {
            debug!(msg = ?other, "Unhandled wire message type on bridge");
        }
    }
}
