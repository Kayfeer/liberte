use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;

use liberte_shared::premium;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct PremiumStatus {
    pub is_premium: bool,
    pub valid_until: Option<String>,
}

#[tauri::command]
pub fn check_premium(state: State<'_, Arc<Mutex<AppState>>>) -> Result<PremiumStatus, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    Ok(PremiumStatus {
        is_premium: guard.is_premium,
        valid_until: None, // TODO: store expiry in state
    })
}

#[tauri::command]
pub fn activate_premium(
    state: State<'_, Arc<Mutex<AppState>>>,
    token_json: String,
) -> Result<PremiumStatus, String> {
    let token: premium::PremiumToken =
        serde_json::from_str(&token_json).map_err(|e| format!("Invalid token JSON: {e}"))?;

    let is_valid = premium::check_premium_status(&token);

    if !is_valid {
        return Err("Premium token is invalid or expired".into());
    }

    {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(ref identity) = guard.identity {
            if token.user_pubkey != identity.public_key_bytes() {
                return Err("Token does not match current identity".into());
            }
        } else {
            return Err("No identity loaded".into());
        }
    }

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.is_premium = true;

    info!(
        valid_until = %token.valid_until.to_rfc3339(),
        "Premium activated"
    );

    Ok(PremiumStatus {
        is_premium: true,
        valid_until: Some(token.valid_until.to_rfc3339()),
    })
}
