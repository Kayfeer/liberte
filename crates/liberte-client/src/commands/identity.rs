use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tracing::info;

use liberte_shared::identity::Identity;
use liberte_store::Database;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfoDto {
    pub public_key: String,
    pub short_id: String,
    pub created_at: String,
    pub display_name: Option<String>,
}

fn make_identity_dto(identity: &Identity, display_name: Option<String>) -> IdentityInfoDto {
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    let short_id = format!(
        "{}…{}",
        &pubkey_hex[..8],
        &pubkey_hex[pubkey_hex.len() - 8..]
    );
    IdentityInfoDto {
        public_key: pubkey_hex,
        short_id,
        created_at: chrono::Utc::now().to_rfc3339(),
        display_name,
    }
}

#[tauri::command]
pub fn create_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
    display_name: Option<String>,
) -> Result<IdentityInfoDto, String> {
    let identity = Identity::generate();
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    let db_key = identity.derive_db_key();

    // Sanitize display name
    let display_name = display_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());

    info!(pubkey = %pubkey_hex, name = ?display_name, "Creating new identity");

    let db = Database::new(&db_key).map_err(|e| format!("Failed to open database: {e}"))?;

    let user = liberte_store::User {
        pubkey: identity.public_key_bytes(),
        display_name: display_name.clone(),
        avatar_hash: None,
        created_at: chrono::Utc::now(),
    };

    let _ = db.conn().execute(
        "INSERT OR IGNORE INTO users (pubkey, display_name, avatar_hash, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            hex::encode(user.pubkey),
            user.display_name,
            user.avatar_hash,
            user.created_at.to_rfc3339(),
        ],
    );

    // persist secret key for reload
    let export = identity.to_export();
    let secret_hex = hex::encode(export.secret_key);
    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS local_identity (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            secret_key TEXT NOT NULL
        );",
    );
    let _ = db.conn().execute(
        "INSERT OR REPLACE INTO local_identity (id, secret_key) VALUES (1, ?1)",
        rusqlite::params![secret_hex],
    );

    // Also save display name in app_settings
    if display_name.is_some() {
        let settings = crate::commands::settings::AppSettings {
            display_name: display_name.clone(),
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap_or_default();
        let _ = db.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                json TEXT NOT NULL
            );",
        );
        let _ = db.conn().execute(
            "INSERT OR REPLACE INTO app_settings (id, json) VALUES (1, ?1)",
            rusqlite::params![json],
        );
    }

    let dto = make_identity_dto(&identity, display_name);

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(dto)
}

#[tauri::command]
pub fn load_identity(state: State<'_, Arc<Mutex<AppState>>>) -> Result<IdentityInfoDto, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    if let Some(ref id) = guard.identity {
        // Read display name from settings
        let display_name = guard.database.as_ref().and_then(read_display_name);
        return Ok(make_identity_dto(id, display_name));
    }
    drop(guard);

    // Step 1: Open DB with a temporary key to read the stored secret.
    // The DB is not encrypted with SQLCipher yet (plain rusqlite), so any
    // key works for opening. Once we recover the identity we derive the
    // real DB key and could re-open if needed.
    let bootstrap_key = blake3::hash(b"liberte-bootstrap-db-open-v1");
    let bootstrap_arr: [u8; 32] = *bootstrap_key.as_bytes();
    let db = Database::new(&bootstrap_arr)
        .map_err(|e| format!("Failed to open database (is identity created?): {e}"))?;

    let secret_hex: String = db
        .conn()
        .query_row(
            "SELECT secret_key FROM local_identity WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("No identity found in database: {e}"))?;

    let secret_bytes =
        hex::decode(&secret_hex).map_err(|e| format!("Corrupt identity data: {e}"))?;

    if secret_bytes.len() != 32 {
        return Err("Corrupt identity: expected 32-byte secret key".into());
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&secret_bytes);
    let identity = Identity::from_secret_bytes(&key);
    let pubkey_hex = hex::encode(identity.public_key_bytes());

    info!(pubkey = %pubkey_hex, "Loaded existing identity");

    let display_name = read_display_name(&db);
    let dto = make_identity_dto(&identity, display_name);

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(dto)
}

/// Read the display name from app_settings JSON, falling back to users table.
fn read_display_name(db: &Database) -> Option<String> {
    // Try app_settings first
    if let Ok(json_str) =
        db.conn()
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
    {
        if let Ok(settings) =
            serde_json::from_str::<crate::commands::settings::AppSettings>(&json_str)
        {
            if settings.display_name.is_some() {
                return settings.display_name;
            }
        }
    }
    // Fallback: users table (own pubkey row)
    db.conn()
        .query_row("SELECT display_name FROM users LIMIT 1", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .ok()
        .flatten()
}

#[tauri::command]
pub fn export_pubkey(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    Ok(hex::encode(identity.public_key_bytes()))
}

/// Update the display name for the current user.
#[tauri::command]
pub fn set_display_name(
    state: State<'_, Arc<Mutex<AppState>>>,
    name: String,
) -> Result<(), String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let name = name.trim().to_string();
    let display_name = if name.is_empty() { None } else { Some(name) };

    // Update users table
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.conn()
        .execute(
            "UPDATE users SET display_name = ?1 WHERE pubkey = ?2",
            rusqlite::params![display_name, pubkey_hex],
        )
        .map_err(|e| format!("Failed to update user: {e}"))?;

    // Also update app_settings
    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );",
    );

    // Read existing settings and update display_name
    let current: crate::commands::settings::AppSettings = db
        .conn()
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    let updated = crate::commands::settings::AppSettings {
        display_name: display_name.clone(),
        ..current
    };

    let json = serde_json::to_string(&updated).map_err(|e| format!("Serialization failed: {e}"))?;
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO app_settings (id, json) VALUES (1, ?1)",
            rusqlite::params![json],
        )
        .map_err(|e| format!("Failed to save settings: {e}"))?;

    info!(name = ?display_name, "Display name updated");
    Ok(())
}
