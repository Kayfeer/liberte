use std::sync::{Arc, Mutex};

use tauri::State;
use tracing::info;

use liberte_shared::identity::Identity;
use liberte_store::Database;

use crate::state::AppState;

#[tauri::command]
pub fn create_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let identity = Identity::generate();
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    let db_key = identity.derive_db_key();

    info!(pubkey = %pubkey_hex, "Creating new identity");

    let db = Database::new(&db_key).map_err(|e| format!("Failed to open database: {e}"))?;

    let user = liberte_store::User {
        pubkey: identity.public_key_bytes(),
        display_name: None,
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

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(pubkey_hex)
}

#[tauri::command]
pub fn load_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    if let Some(ref id) = guard.identity {
        return Ok(hex::encode(id.public_key_bytes()));
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

    let secret_bytes = hex::decode(&secret_hex)
        .map_err(|e| format!("Corrupt identity data: {e}"))?;

    if secret_bytes.len() != 32 {
        return Err("Corrupt identity: expected 32-byte secret key".into());
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&secret_bytes);
    let identity = Identity::from_secret_bytes(&key);
    let pubkey_hex = hex::encode(identity.public_key_bytes());

    info!(pubkey = %pubkey_hex, "Loaded existing identity");

    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(pubkey_hex)
}

#[tauri::command]
pub fn export_pubkey(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    Ok(hex::encode(identity.public_key_bytes()))
}
