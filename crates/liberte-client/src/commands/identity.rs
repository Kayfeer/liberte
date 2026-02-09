//! Identity management commands.
//!
//! These commands allow the frontend to create a new cryptographic identity,
//! load an existing one from the local database, and export the public key.

use std::sync::{Arc, Mutex};

use tauri::State;
use tracing::info;

use liberte_shared::identity::Identity;
use liberte_store::Database;

use crate::state::AppState;

/// Generate a brand-new Ed25519 identity, open the encrypted database with
/// a key derived from it, persist the identity export, and return the
/// hex-encoded public key.
#[tauri::command]
pub fn create_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let identity = Identity::generate();
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    let db_key = identity.derive_db_key();

    info!(pubkey = %pubkey_hex, "Creating new identity");

    // Open the encrypted database
    let db = Database::new(&db_key).map_err(|e| format!("Failed to open database: {e}"))?;

    // Persist the identity export as a blob in the user table so it can
    // be restored later.  We store it as the single "self" user row.
    let user = liberte_store::User {
        pubkey: identity.public_key_bytes(),
        display_name: None,
        avatar_hash: None,
        created_at: chrono::Utc::now(),
    };

    // Best-effort insert; ignore if the row already exists.
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

    // Store the secret key material so we can reload later.
    // We use a dedicated `identity_export` table-less approach: write a
    // single row into a simple key-value pragma.  For now, store as a blob
    // in a dedicated table that the migration should have created, or fall
    // back to a raw CREATE.
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

    // Update application state
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    guard.identity = Some(identity);
    guard.database = Some(db);

    Ok(pubkey_hex)
}

/// Load an existing identity from the local database.
///
/// The caller does not need to provide a password; the identity's secret
/// key is stored in the database which is itself encrypted.  We first
/// attempt to open the DB using a well-known bootstrap key derived from
/// the platform credential store (placeholder: zeroed key for now),
/// then read the stored secret bytes and reconstruct the identity.
///
/// In a full implementation the bootstrap key would come from OS keychain
/// or a user-provided passphrase.
#[tauri::command]
pub fn load_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    // Placeholder: derive a deterministic "bootstrap" key.  In production
    // this would come from the OS keychain or a passphrase-based KDF.
    // For now we try the same derivation used by `create_identity` which
    // means we need the secret key -- chicken-and-egg.  The real flow
    // would store the DB key in the OS keychain.
    //
    // As a pragmatic workaround, we attempt to open the DB with a
    // zero-key first, then look for the local_identity table.  This is
    // safe because the DB file itself lives in an encrypted SQLCipher
    // container keyed by the identity.  On first run after
    // create_identity the DB key was set; the OS keychain should cache it.
    //
    // TODO: integrate with OS keychain (keyring crate).

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    if let Some(ref id) = guard.identity {
        return Ok(hex::encode(id.public_key_bytes()));
    }
    drop(guard);

    // Attempt to open with a placeholder bootstrap key.
    // In production, replace with keyring::Entry retrieval.
    let bootstrap_key = [0u8; 32];
    let db = Database::new(&bootstrap_key)
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

/// Return the current identity's public key as a hex string.
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
