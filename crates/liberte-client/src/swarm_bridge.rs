use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::AppHandle;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use liberte_net::{SwarmCommand, SwarmNotification};
use liberte_shared::crypto;
use liberte_shared::protocol::{ReplayGuard, WireMessage};
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
    // This constructor accepts a 32-byte secret seed, not the expanded 64-byte keypair.
    let seed = blake3::derive_key("liberte-libp2p-keypair-v1", &identity_secret);
    let libp2p_keypair = libp2p::identity::Keypair::ed25519_from_bytes(seed)
        .map_err(|e| format!("Failed to create libp2p keypair: {e}"))?;

    let config = liberte_net::swarm::SwarmConfig {
        extra_dials: crate::commands::network::saved_peer_addresses(&state)?,
        ..Default::default()
    };

    let (cmd_tx, notif_rx, local_peer_id) = liberte_net::spawn_swarm(libp2p_keypair, config)
        .await
        .map_err(|e| format!("Failed to spawn swarm: {e}"))?;

    info!(peer_id = %local_peer_id, "Swarm started");

    // Store cmd_tx in AppState
    {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard.swarm_cmd_tx = Some(cmd_tx.clone());
        guard.network_error = None;
    }

    // Start draining notifications before queueing subscriptions. Otherwise a
    // full notification queue can prevent the swarm from accepting commands.
    let state_clone = state.clone();
    tokio::spawn(async move {
        notification_loop(app, state_clone, notif_rx).await;
    });

    // Subscribe to all existing channels
    subscribe_all_channels(&state, &cmd_tx).await;

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
            if let Err(error) = liberte_net::subscribe_topic(cmd_tx, &ChannelId(uuid)).await {
                if let Ok(mut guard) = state.lock() {
                    guard.network_error = Some(format!("Channel subscription failed: {error}"));
                }
                warn!(%error, "Could not subscribe restored channel");
            }
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
    let mut replays = ReplayGuard::default();
    let mut last_profile_announcement: Option<std::time::Instant> = None;

    info!("Swarm notification bridge started");

    while let Some(notification) = notif_rx.recv().await {
        match notification {
            SwarmNotification::PeerConnected { peer_id, address } => {
                if let Ok(mut guard) = state.lock() {
                    guard.network_error = None;
                    guard.connection_mode = if address
                        .iter()
                        .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2pCircuit))
                    {
                        liberte_shared::types::ConnectionMode::Relayed
                    } else {
                        liberte_shared::types::ConnectionMode::Direct
                    };
                }
                info!(peer = %peer_id, addr = %address, "Peer connected (bridge)");
                emit_event(
                    &app,
                    EVENT_PEER_CONNECTED,
                    PeerEventPayload {
                        peer_id: peer_id.to_string(),
                    },
                );
            }

            SwarmNotification::PeerSubscribed { peer_id, topic } => {
                // A transport connection can predate joining a channel. Announce when the
                // remote subscription exists so a late joiner also receives our profile.
                let shared_channel = topic
                    .strip_prefix("channel:")
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .is_some_and(|id| {
                        state
                            .lock()
                            .ok()
                            .and_then(|guard| {
                                guard
                                    .database
                                    .as_ref()
                                    .map(|db| db.get_channel_key(id).is_ok())
                            })
                            .unwrap_or(false)
                    });
                if shared_channel
                    && last_profile_announcement
                        .is_none_or(|last| last.elapsed() >= std::time::Duration::from_secs(1))
                {
                    debug!(peer = %peer_id, %topic, "Announcing profile to channel subscriber");
                    last_profile_announcement = Some(std::time::Instant::now());
                    crate::commands::identity::spawn_profile_broadcast(state.clone());
                }
            }

            SwarmNotification::PeerDisconnected { peer_id } => {
                let tx = state.lock().ok().and_then(|g| g.swarm_cmd_tx.clone());
                if let Some(tx) = tx {
                    if let Ok(snapshot) = liberte_net::network_snapshot(&tx).await {
                        if let Ok(mut guard) = state.lock() {
                            guard.connection_mode = match snapshot.connection_mode.as_str() {
                                "direct" => liberte_shared::types::ConnectionMode::Direct,
                                "relayed" => liberte_shared::types::ConnectionMode::Relayed,
                                _ => liberte_shared::types::ConnectionMode::Disconnected,
                            };
                        }
                    }
                }
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
                handle_incoming_message(&app, &state, &topic, &data, &mut replays);
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
    replays: &mut ReplayGuard,
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

    let verified =
        match WireMessage::open_authenticated(data, ChannelId(channel_uuid), &channel_key) {
            Ok(message) => message,
            Err(e) => {
                debug!(error = %e, "Rejected unauthenticated network message");
                return;
            }
        };
    if !replays.accept(&verified) || own_pubkey == Some(verified.author.0) {
        return;
    }
    let author = verified.author;
    let wire_msg = verified.message;

    match wire_msg {
        WireMessage::ChatMessage(chat) => {
            if crypto::decrypt(&channel_key, &chat.encrypted_content).is_err() {
                return;
            }
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
                        return;
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
                    if !db
                        .get_message_by_id(reaction.message_id)
                        .is_ok_and(|message| message.channel_id == channel_uuid)
                    {
                        return;
                    }
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

        WireMessage::VoiceFrame(frame) => {
            if state
                .lock()
                .ok()
                .and_then(|g| g.call_channel_id.clone())
                .as_deref()
                != Some(channel_uuid_str)
            {
                return;
            }
            // Skip our own voice frames
            if let Some(own_pk) = own_pubkey {
                if frame.sender.0 == own_pk {
                    return;
                }
            }

            // Convert i16 PCM bytes back to f32 samples and upsample 16kHz → 48kHz
            let i16_samples: Vec<i16> = frame
                .audio_data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c))
                .collect();

            let mut f32_samples = Vec::with_capacity(i16_samples.len() * 3);
            for &s in &i16_samples {
                let f = s as f32 / 32767.0;
                // Simple upsample: repeat each sample 3x (16kHz → 48kHz)
                f32_samples.push(f);
                f32_samples.push(f);
                f32_samples.push(f);
            }

            // Send to playback pipeline
            let guard = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(ref tx) = guard.voice_playback_tx {
                let _ = tx.try_send(f32_samples);
            }
        }

        WireMessage::VoiceEvent(event) => {
            let user_hex = event.user_id.to_hex();
            let ch_id = event.channel_id.0.to_string();

            match event.event_type {
                liberte_shared::protocol::VoiceEventType::Join => {
                    info!(user = %user_hex, channel = %ch_id, "Voice peer joined");
                    emit_event(
                        app,
                        EVENT_VOICE_PEER_JOINED,
                        VoicePeerPayload {
                            channel_id: ch_id,
                            user_id: user_hex,
                        },
                    );
                }
                liberte_shared::protocol::VoiceEventType::Leave => {
                    info!(user = %user_hex, channel = %ch_id, "Voice peer left");
                    emit_event(
                        app,
                        EVENT_VOICE_PEER_LEFT,
                        VoicePeerPayload {
                            channel_id: ch_id,
                            user_id: user_hex,
                        },
                    );
                }
                liberte_shared::protocol::VoiceEventType::Mute => {
                    emit_event(
                        app,
                        EVENT_VOICE_PEER_MUTED,
                        VoicePeerMutedPayload {
                            channel_id: ch_id,
                            user_id: user_hex,
                            muted: true,
                        },
                    );
                }
                liberte_shared::protocol::VoiceEventType::Unmute => {
                    emit_event(
                        app,
                        EVENT_VOICE_PEER_MUTED,
                        VoicePeerMutedPayload {
                            channel_id: ch_id,
                            user_id: user_hex,
                            muted: false,
                        },
                    );
                }
            }
        }

        WireMessage::ProfileUpdate(profile) => {
            let user_id = author.to_hex();
            if let Ok(guard) = state.lock() {
                if let Some(db) = guard.database.as_ref() {
                    let result = db.conn().execute(
                        "INSERT INTO users (pubkey, display_name, avatar_hash, bio, status, created_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5) ON CONFLICT(pubkey) DO UPDATE SET display_name = excluded.display_name, bio = excluded.bio, status = excluded.status",
                        rusqlite::params![user_id, profile.display_name, profile.bio, profile.status, profile.timestamp.to_rfc3339()],
                    );
                    if let Err(error) = result {
                        warn!(%error, "Could not store peer profile");
                    }
                }
            }
            #[derive(serde::Serialize, Clone)]
            #[serde(rename_all = "camelCase")]
            struct ProfilePayload {
                user_id: String,
                display_name: Option<String>,
                bio: Option<String>,
                status: String,
            }
            emit_event(
                app,
                "profile-updated",
                ProfilePayload {
                    user_id,
                    display_name: profile.display_name,
                    bio: profile.bio,
                    status: profile.status,
                },
            );
        }
        WireMessage::Signal(signal) => {
            let active = state.lock().ok().and_then(|g| {
                if g.call_mode == "mesh" {
                    g.call_channel_id.clone()
                } else {
                    None
                }
            });
            if own_pubkey != Some(signal.target.0) || active.as_deref() != Some(channel_uuid_str) {
                return;
            }
            #[derive(serde::Serialize, Clone)]
            #[serde(rename_all = "camelCase")]
            struct SignalPayload {
                channel_id: String,
                sender: String,
                target: String,
                signal_type: liberte_shared::protocol::SignalType,
            }
            emit_event(
                app,
                "call-signal",
                SignalPayload {
                    channel_id: channel_uuid.to_string(),
                    sender: author.to_hex(),
                    target: signal.target.to_hex(),
                    signal_type: signal.signal_type,
                },
            );
        }
        message @ (WireMessage::FileOffer(_)
        | WireMessage::FileAccept(_)
        | WireMessage::FileChunk(_)
        | WireMessage::FileComplete(_)) => {
            crate::file_transfer::handle_incoming(
                app,
                state,
                ChannelId(channel_uuid),
                author,
                message,
            );
        }
        other => {
            debug!(msg = ?other, "Unhandled wire message type on bridge");
        }
    }
}
