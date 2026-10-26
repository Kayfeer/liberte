use crate::state::AppState;
use serde::Serialize;
use rusqlite::OptionalExtension;
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub peers: Vec<String>,
    pub connection_mode: String,
    pub listen_addresses: Vec<String>,
    pub external_addresses: Vec<String>,
    pub local_peer_id: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn connect_peer(
    state: State<'_, Arc<Mutex<AppState>>>,
    multiaddr: String,
) -> Result<(), String> {
    if multiaddr.len() > 1024 {
        return Err("Address is too long".into());
    }
    let address = multiaddr
        .parse::<libp2p::Multiaddr>()
        .map_err(|e| format!("Invalid multiaddr: {e}"))?;
    let address = liberte_net::dns::resolve_multiaddr(address)
        .await
        .map_err(|e| e.to_string())?;
    let tx = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.swarm_cmd_tx.clone().ok_or("Network not started")?
    };
    if liberte_net::swarm::target_peer_id(&address).is_some() {
        remember_peer_addresses(state.inner(), std::slice::from_ref(&address))?;
    }
    let result = liberte_net::dial_confirmed(&tx, address).await;
    if let Ok(mut guard) = state.lock() {
        guard.network_error = result.as_ref().err().cloned();
    }
    result
}

#[tauri::command]
pub async fn get_network_status(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<NetworkStatus, String> {
    let (tx, error) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        (guard.swarm_cmd_tx.clone(), guard.network_error.clone())
    };
    if let Some(tx) = tx {
        match liberte_net::network_snapshot(&tx).await {
            Ok(snapshot) => {
                return Ok(NetworkStatus {
                    peers: snapshot.peers,
                    connection_mode: snapshot.connection_mode,
                    listen_addresses: snapshot.listen_addresses,
                    external_addresses: snapshot.external_addresses,
                    local_peer_id: Some(snapshot.local_peer_id),
                    error,
                })
            }
            Err(error) => {
                return Ok(NetworkStatus {
                    peers: vec![],
                    connection_mode: "disconnected".into(),
                    listen_addresses: vec![],
                    external_addresses: vec![],
                    local_peer_id: None,
                    error: Some(error),
                })
            }
        }
    }
    Ok(NetworkStatus {
        peers: vec![],
        connection_mode: "disconnected".into(),
        listen_addresses: vec![],
        external_addresses: vec![],
        local_peer_id: None,
        error,
    })
}

#[tauri::command]
pub async fn list_peers(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Vec<String>, String> {
    Ok(get_network_status(state).await?.peers)
}

#[tauri::command]
pub async fn get_connection_mode(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    Ok(get_network_status(state).await?.connection_mode)
}

/// Persist only explicit connection hints, never every discovered LAN peer.
pub(crate) fn remember_peer_addresses(
    state: &Arc<Mutex<AppState>>,
    addresses: &[libp2p::Multiaddr],
) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let db = guard.database.as_ref().ok_or("Database not opened")?;
    store_peer_addresses(db, addresses)
}

fn store_peer_addresses(db: &liberte_store::Database, addresses: &[libp2p::Multiaddr]) -> Result<(), String> {
    let peer = addresses.first().and_then(liberte_net::swarm::target_peer_id)
        .ok_or("A destination peer ID is required")?;
    if addresses.len() > 8 || addresses.iter().any(|address| {
        address.to_string().len() > 1024 || liberte_net::swarm::target_peer_id(address) != Some(peer)
    }) { return Err("Invalid peer address list".into()); }
    let peer = peer.to_string();
    let transaction = db.conn().unchecked_transaction().map_err(|e| e.to_string())?;
    let previous: Option<String> = transaction.query_row(
        "SELECT addresses_json FROM peer_addresses WHERE peer_id = ?1", [&peer], |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    let mut saved = Vec::new();
    for address in addresses {
        let address = address.to_string();
        if !saved.contains(&address) { saved.push(address); }
    }
    if let Some(previous) = previous.filter(|json| json.len() <= 16_384) {
        if let Ok(previous) = serde_json::from_str::<Vec<String>>(&previous) {
            for address in previous {
                if saved.len() == 8 { break; }
                if address.len() <= 1024 && !saved.contains(&address) { saved.push(address); }
            }
        }
    }
    let json = serde_json::to_string(&saved).map_err(|e| e.to_string())?;
    transaction.execute(
        "INSERT INTO peer_addresses (peer_id, addresses_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(peer_id) DO UPDATE SET addresses_json = excluded.addresses_json, updated_at = excluded.updated_at",
        rusqlite::params![peer, json, chrono::Utc::now().timestamp_millis()],
    ).map_err(|e| e.to_string())?;
    transaction.execute(
        "DELETE FROM peer_addresses WHERE peer_id NOT IN (SELECT peer_id FROM peer_addresses ORDER BY updated_at DESC, peer_id LIMIT 128)", [],
    ).map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())
}

pub(crate) fn saved_peer_addresses(state: &Arc<Mutex<AppState>>) -> Result<Vec<libp2p::Multiaddr>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let db = guard.database.as_ref().ok_or("Database not opened")?;
    read_peer_addresses(db)
}

fn read_peer_addresses(db: &liberte_store::Database) -> Result<Vec<libp2p::Multiaddr>, String> {
    let mut statement = db.conn().prepare(
        "SELECT peer_id, addresses_json FROM peer_addresses ORDER BY updated_at DESC, peer_id LIMIT 128",
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut addresses = Vec::new();
    for row in rows {
        let (peer, json) = row.map_err(|e| e.to_string())?;
        if json.len() > 16_384 { continue; }
        let Ok(saved) = serde_json::from_str::<Vec<String>>(&json) else { continue; };
        for raw in saved.into_iter().take(8) {
            if raw.len() > 1024 { continue; }
            let Ok(address) = raw.parse::<libp2p::Multiaddr>() else { continue; };
            if liberte_net::swarm::target_peer_id(&address).is_some_and(|target| target.to_string() == peer) {
                addresses.push(address);
            }
        }
    }
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_peer_cache_survives_database_reopen_and_rejects_mixed_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("peers.db");
        let peer = libp2p::PeerId::random();
        let first: libp2p::Multiaddr = format!("/ip4/127.0.0.1/udp/4001/quic-v1/p2p/{peer}").parse().unwrap();
        let other: libp2p::Multiaddr = format!("/ip4/127.0.0.1/udp/4002/quic-v1/p2p/{}", libp2p::PeerId::random()).parse().unwrap();
        {
            let db = liberte_store::Database::open_at(&path, &[0; 32]).unwrap();
            store_peer_addresses(&db, std::slice::from_ref(&first)).unwrap();
            assert!(store_peer_addresses(&db, &[first.clone(), other]).is_err());
        }
        let db = liberte_store::Database::open_at(&path, &[0; 32]).unwrap();
        assert_eq!(read_peer_addresses(&db).unwrap(), vec![first]);
    }
}
