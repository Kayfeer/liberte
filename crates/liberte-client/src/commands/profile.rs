use crate::state::AppState;
use liberte_shared::{
    backup_crypto::{self, BackupKind},
    identity::Identity,
};
use liberte_store::{
    backup::{BackupChannel, BackupPayload},
    Database, StoreError,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::State;

/// Only serialized inside the encrypted profile container.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePayload {
    pub version: String,
    pub exported_at: String,
    pub secret_key_hex: String,
    pub public_key_hex: String,
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<ProfileChannel>,
    #[serde(default)]
    pub backup: Option<BackupPayload>,
}
#[derive(Serialize, Deserialize)]
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

#[tauri::command]
pub fn export_profile(
    state: State<'_, Arc<Mutex<AppState>>>,
    password: Option<String>,
) -> Result<String, String> {
    let password = password
        .as_deref()
        .ok_or("Choose a password for the portable profile")?;
    let plaintext = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let identity = guard.identity.as_ref().ok_or("No identity loaded")?;
        let db = guard.database.as_ref().ok_or("Database not opened")?;
        let backup = db
            .export_backup()
            .map_err(|e| format!("Profile export failed: {e}"))?;
        let display_name = backup
            .users
            .iter()
            .find(|u| u.pubkey == identity.public_key_bytes())
            .and_then(|u| u.display_name.clone());
        let payload = ProfilePayload {
            version: env!("CARGO_PKG_VERSION").into(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            secret_key_hex: hex::encode(identity.secret_bytes()),
            public_key_hex: hex::encode(identity.public_key_bytes()),
            display_name,
            channels: Vec::new(),
            backup: Some(backup),
        };
        serde_json::to_vec(&payload).map_err(|e| e.to_string())?
    };
    backup_crypto::encrypt_password(&plaintext, BackupKind::Profile, password)
}

fn validate_identity(payload: &ProfilePayload) -> Result<Identity, String> {
    chrono::DateTime::parse_from_rfc3339(&payload.exported_at)
        .map_err(|e| format!("Invalid profile date: {e}"))?;
    let secret: [u8; 32] = hex::decode(&payload.secret_key_hex)
        .map_err(|e| format!("Invalid secret key: {e}"))?
        .try_into()
        .map_err(|_| "Profile secret must be 32 bytes")?;
    let public: [u8; 32] = hex::decode(&payload.public_key_hex)
        .map_err(|e| format!("Invalid public key: {e}"))?
        .try_into()
        .map_err(|_| "Profile public key must be 32 bytes")?;
    let identity = Identity::from_secret_bytes(&secret);
    if identity.public_key_bytes() != public {
        return Err("Profile public and secret keys do not match".into());
    }
    Ok(identity)
}

#[tauri::command]
pub async fn save_profile_to_file(
    state: State<'_, Arc<Mutex<AppState>>>,
    file_path: String,
    password: Option<String>,
) -> Result<String, String> {
    let encrypted = export_profile(state, password)?;
    let path = file_path.clone();
    tokio::task::spawn_blocking(move || {
        crate::commands::backup::write_atomic(std::path::Path::new(&path), encrypted.as_bytes())
    })
    .await
    .map_err(|e| format!("Profile export task failed: {e}"))??;
    Ok(file_path)
}

fn profile_backup(payload: &ProfilePayload) -> Result<BackupPayload, String> {
    if let Some(backup) = &payload.backup {
        if !payload.channels.is_empty() {
            return Err("Profile has ambiguous legacy and current channel lists".into());
        }
        return Ok(backup.clone());
    }
    let mut keys = HashMap::new();
    let mut channels = Vec::new();
    for channel in &payload.channels {
        if !channel.key_hex.is_empty() {
            keys.insert(channel.id.clone(), channel.key_hex.clone());
        }
        channels.push(BackupChannel {
            id: channel.id.clone(),
            name: channel.name.clone(),
            server_id: None,
            created_at: payload.exported_at.clone(),
        });
    }
    Ok(BackupPayload {
        version: payload.version.clone(),
        created_at: payload.exported_at.clone(),
        channels,
        messages: Vec::new(),
        channel_keys: keys,
        servers: Vec::new(),
        users: Vec::new(),
        reactions: Vec::new(),
    })
}

fn restore_profile(
    db: &Database,
    payload: &ProfilePayload,
    identity: &Identity,
) -> Result<usize, String> {
    let backup = profile_backup(payload)?;
    let public_key = hex::encode(identity.public_key_bytes());
    let secret_key = hex::encode(identity.secret_bytes());
    let stats = db.import_backup_with(&backup, |conn| {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS local_identity (id INTEGER PRIMARY KEY CHECK(id=1), secret_key TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS app_settings (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);")?;
        let existing: Option<String> = conn.query_row("SELECT secret_key FROM local_identity WHERE id=1", [], |row| row.get(0)).optional()?;
        if let Some(existing) = existing {
            if existing.to_lowercase() != secret_key { return Err(StoreError::InvalidBackup("Another local identity exists. Import into a new installation to preserve it.".into())); }
        }
        conn.execute("INSERT INTO local_identity (id,secret_key) VALUES(1,?1) ON CONFLICT(id) DO NOTHING", params![secret_key])?;
        conn.execute("INSERT INTO users (pubkey,display_name,avatar_hash,bio,status,created_at) VALUES(?1,?2,NULL,NULL,'online',?3) ON CONFLICT(pubkey) DO NOTHING",
            params![public_key, payload.display_name, payload.exported_at])?;
        if let Some(name) = &payload.display_name {
            conn.execute("UPDATE users SET display_name=?1 WHERE pubkey=?2", params![name, public_key])?;
            let current: Option<String> = conn.query_row("SELECT json FROM app_settings WHERE id=1", [], |row| row.get(0)).optional()?;
            let mut settings = match current {
                Some(json) => serde_json::from_str::<serde_json::Value>(&json).map_err(|e| StoreError::InvalidBackup(format!("Existing settings are invalid: {e}")))?,
                None => serde_json::to_value(crate::commands::settings::AppSettings::default()).map_err(|e| StoreError::InvalidBackup(e.to_string()))?,
            };
            let object = settings.as_object_mut().ok_or_else(|| StoreError::InvalidBackup("Existing settings are not an object".into()))?;
            object.insert("displayName".into(), name.clone().into());
            conn.execute("INSERT INTO app_settings (id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json", params![settings.to_string()])?;
        }
        Ok(())
    }).map_err(|e| format!("Profile import failed; existing data preserved: {e}"))?;
    Ok(stats.channels_imported)
}

/// Import only while logged out. The caller then invokes load_identity, which
/// starts the network with the restored key instead of retaining a previous swarm.
#[tauri::command]
pub fn import_profile(
    state: State<'_, Arc<Mutex<AppState>>>,
    json: String,
    password: Option<String>,
    allow_legacy: Option<bool>,
) -> Result<ProfileImportResult, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    if guard.identity.is_some() {
        return Err(
            "Import a portable profile from the welcome screen on a new installation".into(),
        );
    }
    let plaintext = backup_crypto::decode_import(
        &json,
        password.as_deref(),
        None,
        BackupKind::Profile,
        allow_legacy.unwrap_or(false),
    )?;
    let payload: ProfilePayload =
        serde_json::from_slice(&plaintext).map_err(|e| format!("Invalid profile contents: {e}"))?;
    let identity = validate_identity(&payload)?;
    let db = Database::new(&identity.derive_db_key())
        .map_err(|e| format!("Cannot open profile database: {e}"))?;
    let channels_imported = restore_profile(&db, &payload, &identity)?;
    // Keep state unloaded. load_identity will open the committed database and start P2P.
    Ok(ProfileImportResult {
        public_key: hex::encode(identity.public_key_bytes()),
        channels_imported,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(identity: &Identity) -> ProfilePayload {
        ProfilePayload {
            version: "test".into(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            secret_key_hex: hex::encode(identity.secret_bytes()),
            public_key_hex: hex::encode(identity.public_key_bytes()),
            display_name: Some("Restored".into()),
            channels: Vec::new(),
            backup: None,
        }
    }
    #[test]
    fn rejects_public_key_mismatch() {
        let mut profile = payload(&Identity::generate());
        profile.public_key_hex = hex::encode([0; 32]);
        assert!(validate_identity(&profile).is_err());
    }
    #[test]
    fn portable_profile_restores_and_cannot_overwrite_another_identity() {
        let dir = tempfile::tempdir().unwrap();
        let identity = Identity::generate();
        let source =
            Database::open_at(&dir.path().join("source.db"), &identity.derive_db_key()).unwrap();
        let channel = liberte_store::Channel {
            id: uuid::Uuid::new_v4(),
            name: "Restored history".into(),
            server_id: None,
            created_at: chrono::Utc::now(),
        };
        source.create_channel(&channel).unwrap();
        source
            .store_channel_key(channel.id, &hex::encode([9; 32]))
            .unwrap();
        let message = liberte_store::Message {
            id: uuid::Uuid::new_v4(),
            channel_id: channel.id,
            sender_pubkey: identity.public_key_bytes(),
            encrypted_content: liberte_shared::crypto::encrypt(&[9; 32], b"migrated message")
                .unwrap(),
            timestamp: chrono::Utc::now(),
        };
        source.insert_message(&message).unwrap();
        let mut profile_payload = payload(&identity);
        profile_payload.backup = Some(source.export_backup().unwrap());
        let db =
            Database::open_at(&dir.path().join("profile.db"), &identity.derive_db_key()).unwrap();
        let exported = backup_crypto::encrypt_password(
            &serde_json::to_vec(&profile_payload).unwrap(),
            BackupKind::Profile,
            "portable profile password",
        )
        .unwrap();
        let decrypted = backup_crypto::decode_import(
            &exported,
            Some("portable profile password"),
            None,
            BackupKind::Profile,
            false,
        )
        .unwrap();
        let profile: ProfilePayload = serde_json::from_slice(&decrypted).unwrap();
        restore_profile(&db, &profile, &validate_identity(&profile).unwrap()).unwrap();
        let restored = db.get_message_by_id(message.id).unwrap();
        let key: [u8; 32] = hex::decode(db.get_channel_key(channel.id).unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            liberte_shared::crypto::decrypt(&key, &restored.encrypted_content).unwrap(),
            b"migrated message"
        );
        let other = Identity::generate();
        assert!(restore_profile(&db, &payload(&other), &other).is_err());
        let stored: String = db
            .conn()
            .query_row(
                "SELECT secret_key FROM local_identity WHERE id=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, hex::encode(identity.secret_bytes()));
    }
}
