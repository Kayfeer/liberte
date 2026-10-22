use crate::state::AppState;
use serde::Serialize;
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
