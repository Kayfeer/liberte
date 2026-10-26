use std::sync::{Arc, Mutex};

use rusqlite::OptionalExtension;
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
    pub bio: Option<String>,
    pub status: String,
}

fn make_identity_dto(
    identity: &Identity,
    display_name: Option<String>,
    bio: Option<String>,
    status: String,
) -> IdentityInfoDto {
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
        bio,
        status,
    }
}

/// Commit the complete local identity together. A failed profile/settings write
/// must not leave a secret key that prevents the user from retrying creation.
fn persist_new_identity(
    db: &mut Database,
    identity: &Identity,
    display_name: Option<&str>,
) -> Result<(), String> {
    let transaction = db
        .conn_mut()
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| format!("Cannot begin identity creation: {e}"))?;
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS local_identity (
            id INTEGER PRIMARY KEY CHECK (id = 1), secret_key TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL
        );",
        )
        .map_err(|e| format!("Cannot prepare identity storage: {e}"))?;
    let existing: Option<String> = transaction
        .query_row(
            "SELECT secret_key FROM local_identity WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Cannot inspect existing identity: {e}"))?;
    if existing.is_some() {
        return Err("An identity already exists on this device; load it instead".into());
    }
    let current: Option<String> = transaction
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| format!("Cannot read existing settings: {e}"))?;
    let mut settings = match current {
        Some(json) => serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|e| format!("Existing settings are invalid: {e}"))?,
        None => serde_json::to_value(crate::commands::settings::AppSettings::default())
            .map_err(|e| format!("Cannot prepare default settings: {e}"))?,
    };
    serde_json::from_value::<crate::commands::settings::AppSettings>(settings.clone())
        .map_err(|e| format!("Existing settings are invalid: {e}"))?;
    settings
        .as_object_mut()
        .ok_or("Existing settings are not an object")?
        .insert(
            "displayName".into(),
            serde_json::to_value(display_name).map_err(|e| e.to_string())?,
        );
    let settings_json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    let public_key = hex::encode(identity.public_key_bytes());

    transaction
        .execute(
            "INSERT INTO users (pubkey, display_name, avatar_hash, bio, status, created_at)
         VALUES (?1, ?2, NULL, NULL, 'online', ?3)",
            rusqlite::params![public_key, display_name, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| format!("Cannot persist identity profile: {e}"))?;
    transaction
        .execute(
            "INSERT INTO local_identity (id, secret_key) VALUES (1, ?1)",
            [hex::encode(identity.secret_bytes())],
        )
        .map_err(|e| format!("Cannot persist identity key: {e}"))?;
    transaction
        .execute(
            "INSERT INTO app_settings (id, json) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET json=excluded.json",
            [settings_json],
        )
        .map_err(|e| format!("Cannot persist identity settings: {e}"))?;
    transaction
        .commit()
        .map_err(|e| format!("Cannot commit identity creation: {e}"))
}

#[tauri::command]
pub async fn create_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
    display_name: Option<String>,
) -> Result<IdentityInfoDto, String> {
    if state.lock().map_err(|e| e.to_string())?.identity.is_some() {
        return Err("An identity is already loaded".into());
    }
    let identity = Identity::generate();
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    let db_key = identity.derive_db_key();
    let secret_bytes = *identity.secret_bytes();

    // Sanitize display name
    let display_name = display_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    if display_name.as_ref().is_some_and(|n| n.len() > 128) {
        return Err("Display name exceeds 128 bytes".into());
    }

    info!(pubkey = %pubkey_hex, name = ?display_name, "Creating new identity");

    let mut db = Database::new(&db_key).map_err(|e| format!("Failed to open database: {e}"))?;
    persist_new_identity(&mut db, &identity, display_name.as_deref())?;

    let dto = make_identity_dto(&identity, display_name, None, "online".to_string());

    let (app_handle, state_arc) = {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard.identity = Some(identity);
        guard.database = Some(db);
        restore_runtime_settings(&mut guard);
        (guard.app_handle.clone(), state.inner().clone())
    };

    // Start the libp2p swarm and notification bridge (non-blocking: don't fail identity)
    if let Some(app) = app_handle {
        if let Err(e) =
            crate::swarm_bridge::start_swarm_and_bridge(app, state_arc.clone(), secret_bytes).await
        {
            if let Ok(mut guard) = state_arc.lock() {
                guard.network_error = Some(e.clone());
            }
            tracing::error!(error = %e, "Failed to start swarm, continuing without P2P");
        }
    }

    Ok(dto)
}

#[tauri::command]
pub async fn load_identity(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<IdentityInfoDto>, String> {
    // If identity is already loaded and swarm already running, just return the DTO
    let loaded = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(ref id) = guard.identity {
            let display_name = guard
                .database
                .as_ref()
                .and_then(|db| read_display_name(db, id));
            let (bio, status) = guard
                .database
                .as_ref()
                .and_then(|db| read_profile(db, id))
                .unwrap_or((None, "online".to_string()));
            Some((
                make_identity_dto(id, display_name, bio, status),
                *id.secret_bytes(),
                guard.swarm_cmd_tx.is_some(),
                guard.app_handle.clone(),
            ))
        } else {
            None
        }
    };
    if let Some((dto, secret, running, app)) = loaded {
        if !running {
            if let Some(app) = app {
                if let Err(e) =
                    crate::swarm_bridge::start_swarm_and_bridge(app, state.inner().clone(), secret)
                        .await
                {
                    if let Ok(mut guard) = state.lock() {
                        guard.network_error = Some(e);
                    }
                }
            }
        }
        return Ok(Some(dto));
    }

    // Step 1: Open DB with a temporary key to read the stored secret.
    // The DB is not encrypted with SQLCipher yet (plain rusqlite), so any
    // key works for opening. Once we recover the identity we derive the
    // real DB key and could re-open if needed.
    let bootstrap_key = blake3::hash(b"liberte-bootstrap-db-open-v1");
    let bootstrap_arr: [u8; 32] = *bootstrap_key.as_bytes();
    let db = Database::new(&bootstrap_arr)
        .map_err(|e| format!("Failed to open database (is identity created?): {e}"))?;

    let Some(identity) = read_stored_identity(&db)? else {
        return Ok(None);
    };
    let key = *identity.secret_bytes();
    let pubkey_hex = hex::encode(identity.public_key_bytes());

    info!(pubkey = %pubkey_hex, "Loaded existing identity");

    let display_name = read_display_name(&db, &identity);
    let (bio, status) = read_profile(&db, &identity).unwrap_or((None, "online".to_string()));
    let dto = make_identity_dto(&identity, display_name, bio, status);

    let (app_handle, state_arc) = {
        let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        guard.identity = Some(identity);
        guard.database = Some(db);
        restore_runtime_settings(&mut guard);
        (guard.app_handle.clone(), state.inner().clone())
    };

    // Start the libp2p swarm and notification bridge (non-blocking: don't fail identity)
    if let Some(app) = app_handle {
        if let Err(e) =
            crate::swarm_bridge::start_swarm_and_bridge(app, state_arc.clone(), key).await
        {
            if let Ok(mut guard) = state_arc.lock() {
                guard.network_error = Some(e.clone());
            }
            tracing::error!(error = %e, "Failed to start swarm, continuing without P2P");
        }
    }

    Ok(Some(dto))
}

/// A fresh profile has no identity; a damaged existing identity is an error.
fn read_stored_identity(db: &Database) -> Result<Option<Identity>, String> {
    let exists: bool = db
        .conn()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'local_identity')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Cannot inspect local identity: {e}"))?;
    if !exists {
        return Ok(None);
    }
    let secret: Option<String> = db
        .conn()
        .query_row(
            "SELECT secret_key FROM local_identity WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Cannot read local identity: {e}"))?;
    let Some(secret) = secret else {
        return Ok(None);
    };
    let key: [u8; 32] = hex::decode(secret)
        .map_err(|e| format!("Corrupt identity data: {e}"))?
        .try_into()
        .map_err(|_| "Corrupt identity: expected 32-byte secret key")?;
    Ok(Some(Identity::from_secret_bytes(&key)))
}

/// Read the display name from app_settings JSON, falling back to users table.
fn read_display_name(db: &Database, identity: &Identity) -> Option<String> {
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
        .query_row(
            "SELECT display_name FROM users WHERE pubkey = ?1",
            [hex::encode(identity.public_key_bytes())],
            |row| row.get::<_, Option<String>>(0),
        )
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
    if name.len() > 128 {
        return Err("Display name exceeds 128 bytes".into());
    }
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
    spawn_profile_broadcast(state.inner().clone());
    Ok(())
}

/// Read bio and status from the users table for the given identity.
fn read_profile(db: &Database, identity: &Identity) -> Option<(Option<String>, String)> {
    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.get_user_profile(&pubkey_hex).ok()
}

/// Update the bio for the current user.
#[tauri::command]
pub fn set_bio(state: State<'_, Arc<Mutex<AppState>>>, bio: String) -> Result<(), String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let bio_val = bio.trim().to_string();
    if bio_val.len() > 2000 {
        return Err("Bio exceeds 2000 bytes".into());
    }
    let bio_opt = if bio_val.is_empty() {
        None
    } else {
        Some(bio_val.as_str())
    };

    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.set_user_bio(&pubkey_hex, bio_opt)
        .map_err(|e| format!("Failed to update bio: {e}"))?;

    info!(bio = ?bio_opt, "Bio updated");
    spawn_profile_broadcast(state.inner().clone());
    Ok(())
}

/// Update the status for the current user (online, dnd, idle, invisible).
#[tauri::command]
pub fn set_status(state: State<'_, Arc<Mutex<AppState>>>, status: String) -> Result<(), String> {
    let valid = ["online", "dnd", "idle", "invisible"];
    if !valid.contains(&status.as_str()) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: {}",
            status,
            valid.join(", ")
        ));
    }

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let identity = guard
        .identity
        .as_ref()
        .ok_or_else(|| "No identity loaded".to_string())?;

    let pubkey_hex = hex::encode(identity.public_key_bytes());
    db.set_user_status(&pubkey_hex, &status)
        .map_err(|e| format!("Failed to update status: {e}"))?;

    if let (Some(tx), Ok(keys)) = (guard.swarm_cmd_tx.clone(), db.get_all_channel_keys()) {
        let identity = identity.clone();
        let broadcast_status = status.clone();
        tauri::async_runtime::spawn(async move {
            for (channel, key_hex) in keys {
                let Ok(bytes) = hex::decode(&key_hex) else {
                    continue;
                };
                let Ok(key): Result<[u8; 32], _> = bytes.try_into() else {
                    continue;
                };
                let channel = liberte_shared::types::ChannelId(channel);
                let message = liberte_shared::protocol::WireMessage::StatusUpdate(
                    liberte_shared::protocol::StatusUpdate {
                        user_id: identity.user_id(),
                        status: broadcast_status.clone(),
                        timestamp: chrono::Utc::now(),
                    },
                );
                if let Ok(data) = message.seal(channel.clone(), &key, &identity) {
                    let _ = liberte_net::publish_once(&tx, channel.to_topic(), data).await;
                }
            }
        });
    }

    info!(status = %status, "Status updated");
    spawn_profile_broadcast(state.inner().clone());
    Ok(())
}

/// Announce authenticated public profile after subscriptions have propagated.
pub fn spawn_profile_broadcast(state: Arc<Mutex<AppState>>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let context = {
            let Ok(guard) = state.lock() else {
                return;
            };
            let (Some(identity), Some(db), Some(tx)) =
                (&guard.identity, &guard.database, &guard.swarm_cmd_tx)
            else {
                return;
            };
            let display_name = read_display_name(db, identity);
            let (bio, status) = read_profile(db, identity).unwrap_or((None, "online".into()));
            let Ok(keys) = db.get_all_channel_keys() else {
                return;
            };
            (
                identity.clone(),
                tx.clone(),
                keys,
                display_name,
                bio,
                status,
            )
        };
        let (identity, tx, keys, display_name, bio, status) = context;
        for (id, key_hex) in keys {
            let Ok(bytes) = hex::decode(&key_hex) else {
                continue;
            };
            let Ok(key): Result<[u8; 32], _> = bytes.try_into() else {
                continue;
            };
            let channel = liberte_shared::types::ChannelId(id);
            let message = liberte_shared::protocol::WireMessage::ProfileUpdate(
                liberte_shared::protocol::ProfileUpdate {
                    user_id: identity.user_id(),
                    display_name: display_name.clone(),
                    bio: bio.clone(),
                    status: status.clone(),
                    timestamp: chrono::Utc::now(),
                },
            );
            for attempt in 0..3 {
                let Ok(data) = message.seal(channel.clone(), &key, &identity) else {
                    break;
                };
                if liberte_net::publish_once(&tx, channel.to_topic(), data)
                    .await
                    .is_ok()
                {
                    break;
                }
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}

fn restore_runtime_settings(state: &mut AppState) {
    state.server_url = state
        .database
        .as_ref()
        .and_then(|db| {
            db.conn()
                .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                    row.get::<_, String>(0)
                })
                .ok()
        })
        .and_then(|json| serde_json::from_str::<crate::commands::settings::AppSettings>(&json).ok())
        .map(|settings| settings.server_url)
        .unwrap_or_default();
    crate::commands::premium::restore_status(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_identity_profile_or_settings_write_rolls_back_and_can_retry() {
        for (name, trigger) in [
            ("users", "CREATE TRIGGER reject_identity BEFORE INSERT ON users BEGIN SELECT RAISE(ABORT, 'profile write failed'); END;"),
            ("settings", "CREATE TRIGGER reject_identity BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT, 'settings write failed'); END;"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let identity = Identity::generate();
            let peer = Identity::generate();
            let mut db = Database::open_at(&directory.path().join(format!("{name}.db")), &identity.derive_db_key()).unwrap();
            db.conn().execute_batch(
                "CREATE TABLE local_identity (id INTEGER PRIMARY KEY CHECK (id=1), secret_key TEXT NOT NULL);
                 CREATE TABLE app_settings (id INTEGER PRIMARY KEY CHECK (id=1), json TEXT NOT NULL);",
            ).unwrap();
            let previous = r#"{"serverUrl":"https://existing.example.test","notificationsEnabled":false,"displayName":"previous","futurePreference":{"level":2}}"#;
            db.conn().execute("INSERT INTO app_settings VALUES (1,?1)", [previous]).unwrap();
            db.conn().execute(
                "INSERT INTO users (pubkey,display_name,created_at) VALUES (?1,'peer',?2)",
                rusqlite::params![hex::encode(peer.public_key_bytes()), chrono::Utc::now().to_rfc3339()],
            ).unwrap();
            db.conn().execute_batch(trigger).unwrap();
            assert!(persist_new_identity(&mut db, &identity, Some("Created atomically")).is_err());
            assert!(read_stored_identity(&db).unwrap().is_none());
            let users: i64 = db.conn().query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0)).unwrap();
            assert_eq!(users, 1);
            let settings: String = db.conn().query_row("SELECT json FROM app_settings WHERE id=1", [], |row| row.get(0)).unwrap();
            assert_eq!(settings, previous);

            db.conn().execute_batch("DROP TRIGGER reject_identity;").unwrap();
            persist_new_identity(&mut db, &identity, Some("Created atomically")).unwrap();
            assert_eq!(read_stored_identity(&db).unwrap().unwrap().public_key_bytes(), identity.public_key_bytes());
            assert_eq!(read_display_name(&db, &identity).as_deref(), Some("Created atomically"));
            let settings: String = db.conn().query_row("SELECT json FROM app_settings WHERE id=1", [], |row| row.get(0)).unwrap();
            let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
            assert_eq!(settings["serverUrl"], "https://existing.example.test");
            assert_eq!(settings["notificationsEnabled"], false);
            assert_eq!(settings["futurePreference"]["level"], 2);
            assert!(persist_new_identity(&mut db, &peer, Some("Must not replace")).is_err());
            assert_eq!(read_stored_identity(&db).unwrap().unwrap().public_key_bytes(), identity.public_key_bytes());
        }
    }

    #[test]
    fn invalid_existing_settings_do_not_commit_a_partial_identity() {
        let directory = tempfile::tempdir().unwrap();
        let identity = Identity::generate();
        let mut db = Database::open_at(
            &directory.path().join("invalid-settings.db"),
            &identity.derive_db_key(),
        )
        .unwrap();
        db.conn().execute_batch("CREATE TABLE app_settings (id INTEGER PRIMARY KEY, json TEXT NOT NULL); INSERT INTO app_settings VALUES (1,'invalid-json');").unwrap();
        assert!(persist_new_identity(&mut db, &identity, None).is_err());
        assert!(read_stored_identity(&db).unwrap().is_none());
        let users: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap();
        assert_eq!(users, 0);
        let settings: String = db
            .conn()
            .query_row("SELECT json FROM app_settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(settings, "invalid-json");
    }

    #[test]
    fn fresh_identity_is_absent_but_corruption_is_not_silenced() {
        let directory = tempfile::tempdir().unwrap();
        let identity = Identity::generate();
        let db = Database::open_at(
            &directory.path().join("fresh.db"),
            &identity.derive_db_key(),
        )
        .unwrap();
        assert!(read_stored_identity(&db).unwrap().is_none());
        db.conn()
            .execute_batch(
                "CREATE TABLE local_identity (id INTEGER PRIMARY KEY, secret_key TEXT NOT NULL);",
            )
            .unwrap();
        assert!(read_stored_identity(&db).unwrap().is_none());
        db.conn()
            .execute(
                "INSERT INTO local_identity (id, secret_key) VALUES (1, ?1)",
                [hex::encode(identity.secret_bytes())],
            )
            .unwrap();
        assert_eq!(
            read_stored_identity(&db)
                .unwrap()
                .unwrap()
                .public_key_bytes(),
            identity.public_key_bytes()
        );
        db.conn()
            .execute(
                "UPDATE local_identity SET secret_key = 'invalid' WHERE id = 1",
                [],
            )
            .unwrap();
        assert!(read_stored_identity(&db).is_err());
    }

    #[test]
    fn identity_name_fallback_never_borrows_a_peers_name() {
        let directory = tempfile::tempdir().unwrap();
        let own = Identity::generate();
        let peer = Identity::generate();
        let db =
            Database::open_at(&directory.path().join("identity.db"), &own.derive_db_key()).unwrap();
        for (identity, name) in [(&peer, Some("Remote peer")), (&own, None)] {
            db.conn()
                .execute(
                    "INSERT INTO users (pubkey, display_name, created_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![
                        hex::encode(identity.public_key_bytes()),
                        name,
                        chrono::Utc::now().to_rfc3339()
                    ],
                )
                .unwrap();
        }
        assert_eq!(read_display_name(&db, &own), None);
        db.conn()
            .execute(
                "UPDATE users SET display_name = ?1 WHERE pubkey = ?2",
                rusqlite::params!["Own name", hex::encode(own.public_key_bytes())],
            )
            .unwrap();
        assert_eq!(read_display_name(&db, &own).as_deref(), Some("Own name"));
        db.conn()
            .execute(
                "UPDATE users SET display_name = NULL WHERE pubkey = ?1",
                [hex::encode(own.public_key_bytes())],
            )
            .unwrap();
        assert_eq!(read_display_name(&db, &own), None);
    }
}
