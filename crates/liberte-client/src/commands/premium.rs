use crate::state::AppState;
use liberte_shared::premium::{self, PremiumToken};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PremiumStatus {
    pub is_premium: bool,
    pub valid_until: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SavedAccess {
    server: String,
    token: PremiumToken,
    verifying_key: [u8; 32],
}

/// Restore only a credential pinned to this configured server and identity.
pub fn restore_status(state: &mut AppState) {
    state.is_premium = false;
    state.premium_token = None;
    let (Some(db), Some(identity)) = (&state.database, &state.identity) else {
        return;
    };
    let saved = db
        .conn()
        .query_row(
            "SELECT json FROM premium_credentials WHERE id = 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok();
    let Some(saved) = saved.and_then(|json| serde_json::from_str::<SavedAccess>(&json).ok()) else {
        return;
    };
    if saved.server != state.server_url
        || saved.token.user_pubkey != identity.public_key_bytes()
        || !premium::check_premium_status_with_key(&saved.token, &saved.verifying_key)
    {
        return;
    }
    state.is_premium = true;
    state.premium_token = Some(saved.token);
}

#[tauri::command]
pub fn check_premium(state: State<'_, Arc<Mutex<AppState>>>) -> Result<PremiumStatus, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    restore_status(&mut guard);
    Ok(PremiumStatus {
        is_premium: guard.is_premium,
        valid_until: guard
            .premium_token
            .as_ref()
            .map(|t| t.valid_until.to_rfc3339()),
    })
}

#[tauri::command]
pub async fn activate_premium(
    state: State<'_, Arc<Mutex<AppState>>>,
    token_json: String,
) -> Result<PremiumStatus, String> {
    if token_json.len() > 8192 {
        return Err("Jeton trop volumineux".into());
    }
    let token: PremiumToken = serde_json::from_str(&token_json).map_err(|e| e.to_string())?;
    let (server, owner) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        (
            guard.server_url.clone(),
            guard
                .identity
                .as_ref()
                .ok_or("Aucune identité chargée")?
                .public_key_bytes(),
        )
    };
    if token.user_pubkey != owner {
        return Err("Ce jeton appartient à une autre identité".into());
    }
    let key = if server.is_empty() {
        premium::payment_server_pubkey()
    } else {
        let info = crate::commands::settings::get_server_info(server.clone()).await?;
        let hex_key = info
            .payment_server_pubkey
            .ok_or("Ce serveur n'a pas configuré de clé de paiement")?;
        hex::decode(hex_key)
            .map_err(|_| "Clé serveur invalide")?
            .try_into()
            .map_err(|_| "Clé serveur invalide")?
    };
    if !premium::check_premium_status_with_key(&token, &key) {
        return Err("Jeton expiré ou signature invalide".into());
    }
    let status = PremiumStatus {
        is_premium: true,
        valid_until: Some(token.valid_until.to_rfc3339()),
    };
    let saved = serde_json::to_string(&SavedAccess {
        server: server.clone(),
        token: token.clone(),
        verifying_key: key,
    })
    .map_err(|e| e.to_string())?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard.server_url != server
        || guard.identity.as_ref().map(|i| i.public_key_bytes()) != Some(owner)
    {
        return Err("La configuration a changé pendant la validation".into());
    }
    let db = guard.database.as_ref().ok_or("Base indisponible")?;
    db.conn().execute_batch("CREATE TABLE IF NOT EXISTS premium_credentials (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)").map_err(|e| e.to_string())?;
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO premium_credentials(id,json) VALUES(1,?1)",
            rusqlite::params![saved],
        )
        .map_err(|e| e.to_string())?;
    guard.premium_token = Some(token);
    guard.is_premium = true;
    Ok(status)
}
