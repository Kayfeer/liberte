use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::info;

use crate::state::AppState;

/// Portable profile payload for migrating between machines.
/// Contains the secret key (encrypted with a user passphrase in the future)
/// plus all channels and their encryption keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePayload {
    /// App version that produced this export
    pub version: String,
    /// ISO 8601 timestamp
    pub exported_at: String,
    /// Hex-encoded Ed25519 secret key (32 bytes)
    pub secret_key_hex: String,
    /// Hex-encoded Ed25519 public key (32 bytes)
    pub public_key_hex: String,
    /// Display name / pseudo
    pub display_name: Option<String>,
    /// Channel list with keys
    pub channels: Vec<ProfileChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChannel {
    pub id: String,
    pub name: String,
    pub key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileImportResult {
    pub public_key: String,
    pub channels_imported: usize,
}

/// Export identity + channel keys as a portable profile JSON.
#[tauri::command]
pub fn export_profile(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let export = identity.to_export();
    let secret_hex = hex::encode(export.secret_key);
    let public_hex = hex::encode(identity.public_key_bytes());

    let channels_db = db
        .list_channels()
        .map_err(|e| format!("Failed to list channels: {e}"))?;

    let channel_keys = db
        .get_all_channel_keys()
        .map_err(|e| format!("Failed to load channel keys: {e}"))?;

    let channels: Vec<ProfileChannel> = channels_db
        .iter()
        .map(|c| ProfileChannel {
            id: c.id.to_string(),
            name: c.name.clone(),
            key_hex: channel_keys.get(&c.id).cloned().unwrap_or_default(),
        })
        .collect();

    // Read display name from app_settings
    let display_name: Option<String> = db
        .conn()
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|v| v.get("displayName")?.as_str().map(String::from));

    let payload = ProfilePayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        secret_key_hex: secret_hex,
        public_key_hex: public_hex.clone(),
        display_name,
        channels,
    };

    let json =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("Serialization failed: {e}"))?;

    info!(pubkey = %public_hex, "Profile exported");

    Ok(json)
}

/// Import a profile from JSON. Replaces the local identity and adds channels.
#[tauri::command]
pub fn import_profile(
    state: State<'_, Arc<Mutex<AppState>>>,
    json: String,
) -> Result<ProfileImportResult, String> {
    let payload: ProfilePayload =
        serde_json::from_str(&json).map_err(|e| format!("Invalid profile JSON: {e}"))?;

    // Reconstruct identity from secret key
    let secret_bytes =
        hex::decode(&payload.secret_key_hex).map_err(|e| format!("Invalid secret key: {e}"))?;
    if secret_bytes.len() != 32 {
        return Err("Secret key must be 32 bytes".into());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&secret_bytes);

    let identity = liberte_shared::identity::Identity::from_secret_bytes(&key);
    let pubkey_hex = hex::encode(identity.public_key_bytes());

    // Open/create DB for this identity
    let db_key = identity.derive_db_key();
    let db = liberte_store::Database::new(&db_key)
        .map_err(|e| format!("Failed to open database: {e}"))?;

    // Persist the identity secret
    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS local_identity (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            secret_key TEXT NOT NULL
        );",
    );
    let _ = db.conn().execute(
        "INSERT OR REPLACE INTO local_identity (id, secret_key) VALUES (1, ?1)",
        rusqlite::params![payload.secret_key_hex],
    );

    // Import channels and keys
    let mut channels_imported = 0usize;
    for ch in &payload.channels {
        let channel_id =
            uuid::Uuid::parse_str(&ch.id).map_err(|e| format!("Invalid channel id: {e}"))?;
        let now = chrono::Utc::now();

        let channel = liberte_store::Channel {
            id: channel_id,
            name: ch.name.clone(),
            server_id: None,
            created_at: now,
        };

        let _ = db.conn().execute(
            "INSERT OR IGNORE INTO channels (id, name, server_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                channel.id.to_string(),
                channel.name,
                channel.server_id.map(|s| s.to_string()),
                channel.created_at.to_rfc3339(),
            ],
        );

        if !ch.key_hex.is_empty() {
            let _ = db.store_channel_key(channel_id, &ch.key_hex);
        }
        channels_imported += 1;
    }

    // Restore display name if present
    if let Some(ref name) = payload.display_name {
        // Update users table
        let _ = db.conn().execute(
            "UPDATE users SET display_name = ?1 WHERE pubkey = ?2",
            rusqlite::params![name, pubkey_hex],
        );

        // Update app_settings
        let _ = db.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                json TEXT NOT NULL
            );",
        );
        let current: crate::commands::settings::AppSettings = db
            .conn()
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let updated = crate::commands::settings::AppSettings {
            display_name: Some(name.clone()),
            ..current
        };
        if let Ok(json) = serde_json::to_string(&updated) {
            let _ = db.conn().execute(
                "INSERT OR REPLACE INTO app_settings (id, json) VALUES (1, ?1)",
                rusqlite::params![json],
            );
        }
    }

    info!(
        pubkey = %pubkey_hex,
        channels = channels_imported,
        "Profile imported"
    );

    // Update app state
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(ProfileImportResult {
        public_key: pubkey_hex,
        channels_imported,
    })
}
