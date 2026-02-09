//! Network-related Tauri commands.
//!
//! These commands expose peer connectivity operations to the frontend:
//! dialing remote peers, listing connected peers, and querying the
//! current connection mode.

use std::sync::{Arc, Mutex};

use tauri::State;
use tracing::info;

use liberte_net::SwarmCommand;
use liberte_shared::types::ConnectionMode;

use crate::state::AppState;

/// Dial a remote peer at the given multiaddr string.
///
/// The multiaddr is parsed and forwarded to the swarm task via the
/// command channel.  Example: `/ip4/1.2.3.4/udp/4001/quic-v1/p2p/12D3Koo...`
#[tauri::command]
pub async fn connect_peer(
    state: State<'_, Arc<Mutex<AppState>>>,
    multiaddr: String,
) -> Result<(), String> {
    let addr: libp2p::Multiaddr = multiaddr
        .parse()
        .map_err(|e| format!("Invalid multiaddr: {e}"))?;

    let cmd_tx = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard
            .swarm_cmd_tx
            .clone()
            .ok_or_else(|| "Swarm not started".to_string())?
    };

    info!(addr = %addr, "Dialing peer");

    cmd_tx
        .send(SwarmCommand::Dial(addr))
        .await
        .map_err(|e| format!("Failed to send dial command: {e}"))?;

    Ok(())
}

/// Return a list of currently connected peer IDs as hex strings.
///
/// Sends a `GetPeers` command to the swarm task and awaits the response
/// over a oneshot channel.
#[tauri::command]
pub async fn list_peers(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<String>, String> {
    let cmd_tx = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard
            .swarm_cmd_tx
            .clone()
            .ok_or_else(|| "Swarm not started".to_string())?
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

    cmd_tx
        .send(SwarmCommand::GetPeers(reply_tx))
        .await
        .map_err(|e| format!("Failed to send GetPeers command: {e}"))?;

    let peers = reply_rx
        .await
        .map_err(|e| format!("Swarm did not reply: {e}"))?;

    Ok(peers.iter().map(|p| p.to_string()).collect())
}

/// Return the current connection mode as a string: `"direct"`, `"relayed"`,
/// or `"disconnected"`.
#[tauri::command]
pub fn get_connection_mode(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let mode = match guard.connection_mode {
        ConnectionMode::Direct => "direct",
        ConnectionMode::Relayed => "relayed",
        ConnectionMode::Disconnected => "disconnected",
    };

    Ok(mode.to_string())
}
