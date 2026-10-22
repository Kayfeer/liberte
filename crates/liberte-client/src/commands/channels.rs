use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;
use uuid::Uuid;

use liberte_shared::crypto::generate_symmetric_key;
use liberte_shared::invite::InviteToken;
use liberte_shared::types::ChannelId;
use liberte_store::Channel;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct CreateChannelResult {
    pub id: String,
    pub name: String,
    pub channel_key_hex: String,
}

#[tauri::command]
pub async fn create_channel(
    state: State<'_, Arc<Mutex<AppState>>>,
    name: String,
) -> Result<CreateChannelResult, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 128 {
        return Err("Channel name must contain 1 to 128 bytes".into());
    }
    let channel_id = Uuid::new_v4();
    let channel_key = generate_symmetric_key();
    let channel_key_hex = hex::encode(channel_key);
    let now = chrono::Utc::now();

    let channel = Channel {
        id: channel_id,
        name: name.clone(),
        server_id: None,
        created_at: now,
    };

    let cmd_tx = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard
            .database
            .as_ref()
            .ok_or_else(|| "Database not opened".to_string())?;

        db.create_channel(&channel)
            .map_err(|e| format!("Failed to create channel: {e}"))?;

        db.store_channel_key(channel_id, &channel_key_hex)
            .map_err(|e| format!("Failed to store channel key: {e}"))?;

        guard.swarm_cmd_tx.clone()
    };

    // Subscribe to GossipSub topic
    if let Some(tx) = cmd_tx {
        let topic = ChannelId(channel_id).to_topic();
        let _ = tx
            .send(liberte_net::SwarmCommand::SubscribeTopic(topic))
            .await;
    }

    info!(channel_id = %channel_id, name = %name, "Channel created");

    Ok(CreateChannelResult {
        id: channel_id.to_string(),
        name,
        channel_key_hex,
    })
}

#[tauri::command]
pub async fn generate_invite(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
    channel_name: String,
    channel_key_hex: String,
) -> Result<String, String> {
    let channel_uuid =
        Uuid::parse_str(&channel_id).map_err(|e| format!("Invalid channel_id: {e}"))?;

    let key_bytes =
        hex::decode(&channel_key_hex).map_err(|e| format!("Invalid channel key hex: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("Channel key must be 32 bytes".into());
    }
    let mut channel_key = [0u8; 32];
    channel_key.copy_from_slice(&key_bytes);

    let (identity, tx) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard.database.as_ref().ok_or("Database not opened")?;
        let stored = db
            .get_channel_key(channel_uuid)
            .map_err(|e| e.to_string())?;
        if stored != channel_key_hex {
            return Err("Channel key does not match this profile".into());
        }
        (
            guard.identity.clone().ok_or("No identity loaded")?,
            guard.swarm_cmd_tx.clone(),
        )
    };
    if channel_name.trim().is_empty() || channel_name.len() > 128 {
        return Err("Invalid channel name".into());
    }
    let mut addresses = Vec::new();
    if let Some(tx) = tx {
        if let Ok(snapshot) = liberte_net::network_snapshot(&tx).await {
            addresses.extend(snapshot.external_addresses);
            addresses.extend(snapshot.listen_addresses);
            let mut seen = std::collections::HashSet::new();
            addresses.retain(|address| seen.insert(address.clone()));
            addresses.sort_by_key(|address| !address.contains("/p2p-circuit"));
            addresses.truncate(8);
        }
    }
    let token = InviteToken::create_with_addresses(
        &identity,
        channel_uuid,
        channel_name,
        channel_key,
        addresses,
    );
    let code = token.encode();

    info!(channel_id = %channel_id, "Invite generated");

    Ok(code)
}

#[tauri::command]
pub async fn accept_invite(
    state: State<'_, Arc<Mutex<AppState>>>,
    invite_code: String,
) -> Result<CreateChannelResult, String> {
    let token =
        InviteToken::decode(&invite_code).map_err(|e| format!("Invalid invite code: {e}"))?;

    token
        .verify()
        .map_err(|e| format!("Invite verification failed: {e}"))?;

    let channel_id = token.payload.channel_id;
    let channel_name = token.payload.channel_name.clone();
    let channel_key_hex = hex::encode(token.payload.channel_key);
    let now = chrono::Utc::now();

    let channel = Channel {
        id: channel_id,
        name: channel_name.clone(),
        server_id: None,
        created_at: now,
    };

    let cmd_tx = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard
            .database
            .as_ref()
            .ok_or_else(|| "Database not opened".to_string())?;

        // Use INSERT OR IGNORE in case channel already exists
        if let Ok(existing) = db.get_channel_key(channel_id) {
            if existing != channel_key_hex {
                return Err("This invitation conflicts with the existing channel key".into());
            }
        }
        let _ = db.create_channel(&channel);

        db.store_channel_key(channel_id, &channel_key_hex)
            .map_err(|e| format!("Failed to store channel key: {e}"))?;

        guard.swarm_cmd_tx.clone()
    };

    // Subscribe to GossipSub topic
    if let Some(tx) = cmd_tx {
        let topic = ChannelId(channel_id).to_topic();
        let _ = tx
            .send(liberte_net::SwarmCommand::SubscribeTopic(topic))
            .await;
        for raw in &token.payload.addresses {
            if let Ok(address) = raw.parse::<libp2p::Multiaddr>() {
                // Generated invitations contain literal IP addresses; no system-DNS fallback.
                if !address.iter().any(|p| {
                    matches!(
                        p,
                        libp2p::multiaddr::Protocol::Dns(_)
                            | libp2p::multiaddr::Protocol::Dns4(_)
                            | libp2p::multiaddr::Protocol::Dns6(_)
                            | libp2p::multiaddr::Protocol::Dnsaddr(_)
                    )
                }) {
                    let _ = tx.send(liberte_net::SwarmCommand::Dial(address)).await;
                }
            }
        }
    }

    info!(channel_id = %channel_id, name = %channel_name, "Joined channel via invite");
    crate::commands::identity::spawn_profile_broadcast(state.inner().clone());

    Ok(CreateChannelResult {
        id: channel_id.to_string(),
        name: channel_name,
        channel_key_hex,
    })
}

#[tauri::command]
pub fn get_all_channel_keys(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<HashMap<String, String>, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let keys = db
        .get_all_channel_keys()
        .map_err(|e| format!("Failed to load channel keys: {e}"))?;

    // Convert Uuid keys to String for serialization
    Ok(keys
        .into_iter()
        .map(|(id, k)| (id.to_string(), k))
        .collect())
}
