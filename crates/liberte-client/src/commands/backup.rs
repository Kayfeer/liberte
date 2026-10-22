use std::{io::Write, path::{Path, PathBuf}, sync::{Arc, Mutex}};
use liberte_shared::backup_crypto::{self, BackupKind};
use liberte_store::backup::ImportStats;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::info;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub auto_backup_enabled: bool,
    pub interval_minutes: u32,
    pub backup_dir: String,
    pub server_sync_enabled: bool,
}
impl Default for BackupConfig {
    fn default() -> Self {
        Self { auto_backup_enabled: true, interval_minutes: 30, backup_dir: String::new(), server_sync_enabled: false }
    }
}

/// Portable exports always require a password. No plaintext key material is returned.
#[tauri::command]
pub fn export_backup(state: State<'_, Arc<Mutex<AppState>>>, password: Option<String>) -> Result<String, String> {
    let password = password.as_deref().ok_or("Choose a password for the portable backup")?;
    let plaintext = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard.database.as_ref().ok_or("Database not opened")?;
        serde_json::to_vec(&db.export_backup().map_err(|e| format!("Export failed: {e}"))?).map_err(|e| e.to_string())?
    };
    backup_crypto::encrypt_password(&plaintext, BackupKind::Backup, password)
}

/// Replace a backup atomically, keeping the previous file intact if writing fails.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("Cannot create backup file: {e}"))?;
    temporary.write_all(contents).map_err(|e| format!("Cannot write backup: {e}"))?;
    temporary.as_file().sync_all().map_err(|e| format!("Cannot flush backup: {e}"))?;
    temporary.persist(path).map_err(|e| format!("Cannot save backup atomically: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn save_backup_to_file(state: State<'_, Arc<Mutex<AppState>>>, file_path: String, password: Option<String>) -> Result<String, String> {
    let json = export_backup(state, password)?;
    let path = file_path.clone();
    tokio::task::spawn_blocking(move || write_atomic(Path::new(&path), json.as_bytes())).await
        .map_err(|e| format!("Backup task failed: {e}"))??;
    Ok(file_path)
}

/// Unattended snapshots are encrypted with the original identity. A portable
/// profile is required to recover this key after losing the installation.
#[tauri::command]
pub async fn auto_backup(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let json = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard.database.as_ref().ok_or("Database not opened")?;
        let identity = guard.identity.as_ref().ok_or("No identity loaded")?;
        let plaintext = serde_json::to_vec(&db.export_backup().map_err(|e| format!("Export failed: {e}"))?).map_err(|e| e.to_string())?;
        backup_crypto::encrypt_identity(&plaintext, BackupKind::Backup, identity)?
    };
    let dirs = directories::ProjectDirs::from("com", "liberte", "liberte").ok_or("Cannot determine data directory")?;
    let backup_dir = dirs.data_dir().join("backups");
    tokio::fs::create_dir_all(&backup_dir).await.map_err(|e| format!("Cannot create backup directory: {e}"))?;
    let path = backup_dir.join(format!("liberte_backup_{}_{}.liberte", chrono::Utc::now().format("%Y%m%d_%H%M%S"), uuid::Uuid::new_v4()));
    let output = path.to_string_lossy().into_owned();
    tokio::task::spawn_blocking(move || write_atomic(&path, json.as_bytes())).await.map_err(|e| format!("Backup task failed: {e}"))??;
    if let Err(error) = cleanup_old_backups(&backup_dir, 10).await {
        tracing::warn!(%error, "Backup saved but retention cleanup failed");
    }
    info!(path = %output, "Encrypted automatic backup saved");
    Ok(output)
}

#[tauri::command]
pub async fn import_backup(state: State<'_, Arc<Mutex<AppState>>>, json: String, password: Option<String>, allow_legacy: Option<bool>) -> Result<ImportStats, String> {
    let (stats, channels) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let plaintext = backup_crypto::decode_import(&json, password.as_deref(), guard.identity.as_ref(), BackupKind::Backup, allow_legacy.unwrap_or(false))?;
        let payload: liberte_store::BackupPayload = serde_json::from_slice(&plaintext).map_err(|e| format!("Invalid backup contents: {e}"))?;
        let db = guard.database.as_ref().ok_or("Database not opened")?;
        let stats = db.import_backup(&payload).map_err(|e| format!("Import failed; existing data preserved: {e}"))?;
        (stats, payload.channels)
    };
    subscribe_restored_channels(state.inner(), &channels).await;
    info!(channels = stats.channels_imported, messages = stats.messages_imported, "Encrypted backup imported");
    Ok(stats)
}

async fn subscribe_restored_channels(state: &Arc<Mutex<AppState>>, channels: &[liberte_store::backup::BackupChannel]) {
    let sender = state.lock().ok().and_then(|guard| guard.swarm_cmd_tx.clone());
    let Some(sender) = sender else { return; };
    for channel in channels {
        // The importer has validated every UUID before committing.
        let Ok(id) = uuid::Uuid::parse_str(&channel.id) else { continue; };
        if let Err(error) = liberte_net::subscribe_topic(&sender, &liberte_shared::types::ChannelId(id)).await {
            let message = format!("History restored, but channel subscription failed: {error}");
            tracing::warn!(%message);
            if let Ok(mut guard) = state.lock() { guard.network_error = Some(message); }
            break;
        }
    }
}

#[tauri::command]
pub async fn list_backups() -> Result<Vec<BackupFileInfo>, String> {
    let dirs = directories::ProjectDirs::from("com", "liberte", "liberte").ok_or("Cannot determine data directory")?;
    let backup_dir = dirs.data_dir().join("backups");
    let mut dir = match tokio::fs::read_dir(&backup_dir).await {
        Ok(dir) => dir, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Cannot read backup directory: {e}")),
    };
    let mut entries = Vec::new();
    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        let extension = path.extension().and_then(|e| e.to_str());
        if extension != Some("json") && extension != Some("liberte") { continue; }
        let metadata = entry.metadata().await.map_err(|e| e.to_string())?;
        if !metadata.is_file() { continue; }
        let modified: chrono::DateTime<chrono::Utc> = metadata.modified().map_err(|e| e.to_string())?.into();
        entries.push(BackupFileInfo { file_name: entry.file_name().to_string_lossy().into_owned(), file_path: path.to_string_lossy().into_owned(),
            size_bytes: metadata.len(), modified: modified.to_rfc3339() });
    }
    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileInfo { pub file_name: String, pub file_path: String, pub size_bytes: u64, pub modified: String }

fn is_automatic_backup(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else { return false; };
    let Some(stem) = name.strip_prefix("liberte_backup_").and_then(|s| s.strip_suffix(".liberte")) else { return false; };
    let Some((timestamp, id)) = stem.rsplit_once('_') else { return false; };
    chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%d_%H%M%S").is_ok() && uuid::Uuid::parse_str(id).is_ok()
}

#[derive(Serialize, Deserialize)]
struct BackupSyncRequest { user_pubkey_hex: String, encrypted_data: String }

#[derive(Deserialize)]
struct BackupSyncAck { stored: bool, size_bytes: usize }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupInfo { size_bytes: usize, synced_at: String }

fn remote_context(state: &AppState) -> Result<(String, liberte_shared::identity::Identity, Option<liberte_shared::premium::PremiumToken>), String> {
    let identity = state.identity.as_ref().ok_or("No identity loaded")?.clone();
    if state.server_url.trim().is_empty() { return Err("Configure a server in Settings before syncing backups".into()); }
    Ok((state.server_url.clone(), identity, state.premium_token.clone()))
}

async fn read_limited(mut response: reqwest::Response, maximum: usize) -> Result<Vec<u8>, String> {
    if response.content_length().is_some_and(|length| length > maximum as u64) {
        return Err("Remote backup response exceeds the size limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Backup download interrupted: {e}"))? {
        if chunk.len() > maximum.saturating_sub(bytes.len()) { return Err("Remote backup response exceeds the size limit".into()); }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn send_remote_backup(base: &str, identity: &liberte_shared::identity::Identity,
    premium: Option<&liberte_shared::premium::PremiumToken>, encrypted_data: String) -> Result<RemoteBackupInfo, String> {
    let size = encrypted_data.len();
    // Match the relay's default ceiling; larger backups remain available locally.
    if size > 50 * 1024 * 1024 { return Err("Remote backups are limited to 50 MiB; create a local portable export instead".into()); }
    let body = serde_json::to_vec(&BackupSyncRequest { user_pubkey_hex: hex::encode(identity.public_key_bytes()), encrypted_data })
        .map_err(|e| e.to_string())?;
    let response = crate::server_client::signed_request(base, reqwest::Method::POST, "/backup/sync", body, identity, premium, "application/json").await?;
    let ack: BackupSyncAck = serde_json::from_slice(&read_limited(response, 4096).await?).map_err(|e| format!("Invalid backup acknowledgement: {e}"))?;
    if !ack.stored || ack.size_bytes != size { return Err("Server did not confirm the complete backup".into()); }
    Ok(RemoteBackupInfo { size_bytes: size, synced_at: chrono::Utc::now().to_rfc3339() })
}

async fn fetch_remote_backup(base: &str, identity: &liberte_shared::identity::Identity) -> Result<liberte_store::BackupPayload, String> {
    let path = format!("/backup/{}", hex::encode(identity.public_key_bytes()));
    let response = crate::server_client::signed_request(base, reqwest::Method::GET, &path, Vec::new(), identity, None, "application/octet-stream").await?;
    let bytes = read_limited(response, backup_crypto::MAX_CONTAINER_BYTES).await?;
    let encrypted = std::str::from_utf8(&bytes).map_err(|_| "Invalid remote backup encoding")?;
    // Server data is never eligible for the explicit local legacy-file exception.
    let plaintext = backup_crypto::decode_import(encrypted, None, Some(identity), BackupKind::Backup, false)?;
    serde_json::from_slice(&plaintext).map_err(|e| format!("Invalid remote backup contents: {e}"))
}

#[tauri::command]
pub async fn upload_remote_backup(state: State<'_, Arc<Mutex<AppState>>>) -> Result<RemoteBackupInfo, String> {
    let (base, identity, premium, encrypted) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let (base, identity, premium) = remote_context(&guard)?;
        let payload = guard.database.as_ref().ok_or("Database not opened")?.export_backup().map_err(|e| e.to_string())?;
        let plaintext = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let encrypted = backup_crypto::encrypt_identity(&plaintext, BackupKind::Backup, &identity)?;
        (base, identity, premium, encrypted)
    };
    send_remote_backup(&base, &identity, premium.as_ref(), encrypted).await
}

#[tauri::command]
pub async fn restore_remote_backup(state: State<'_, Arc<Mutex<AppState>>>) -> Result<ImportStats, String> {
    let (base, identity, _) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        remote_context(&guard)?
    };
    let payload = fetch_remote_backup(&base, &identity).await?;
    let stats = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if guard.identity.as_ref().map(|i| i.public_key_bytes()) != Some(identity.public_key_bytes()) {
            return Err("Identity changed during download; retry with the original identity".into());
        }
        guard.database.as_ref().ok_or("Database not opened")?.import_backup(&payload)
            .map_err(|e| format!("Remote restore failed; existing data preserved: {e}"))?
    };
    subscribe_restored_channels(state.inner(), &payload.channels).await;
    Ok(stats)
}
async fn cleanup_old_backups(dir: &Path, keep: usize) -> Result<(), String> {
    let mut rd = tokio::fs::read_dir(dir).await.map_err(|e| e.to_string())?;
    let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    while let Some(entry) = rd.next_entry().await.map_err(|e| e.to_string())? {
        // Never delete plaintext legacy exports, unrelated files, directories or symlinks.
        if !is_automatic_backup(&entry.path()) || !entry.file_type().await.map_err(|e| e.to_string())?.is_file() { continue; }
        files.push((entry.path(), entry.metadata().await.map_err(|e| e.to_string())?.modified().map_err(|e| e.to_string())?));
    }
    files.sort_by(|a,b| b.1.cmp(&a.1));
    for (path, _) in files.into_iter().skip(keep) { tokio::fs::remove_file(path).await.map_err(|e| e.to_string())?; }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn retention_never_matches_legacy_or_unrelated_files() {
        assert!(!is_automatic_backup(Path::new("liberte_backup_20260101_120000.json")));
        assert!(!is_automatic_backup(Path::new("holiday.liberte")));
        assert!(!is_automatic_backup(Path::new("liberte_backup_20260101_120000_bad.liberte")));
        assert!(is_automatic_backup(Path::new(&format!("liberte_backup_20260101_120000_{}.liberte", uuid::Uuid::new_v4()))));
    }

    async fn receive_request(socket: &mut tokio::net::TcpStream) -> (String, String, std::collections::HashMap<String,String>, Vec<u8>) {
        let mut bytes = Vec::new();
        let header_end = loop {
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") { break end + 4; }
            let mut chunk = [0; 4096];
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0); bytes.extend_from_slice(&chunk[..count]);
            assert!(bytes.len() < 1024 * 1024);
        };
        let head = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let mut lines = head.split("\r\n");
        let mut request_line = lines.next().unwrap().split_whitespace();
        let method = request_line.next().unwrap().to_string();
        let path = request_line.next().unwrap().to_string();
        let headers: std::collections::HashMap<_,_> = lines.filter_map(|line| line.split_once(':'))
            .map(|(name,value)| (name.to_lowercase(),value.trim().to_string())).collect();
        let length: usize = headers.get("content-length").map(|s| s.parse().unwrap()).unwrap_or(0);
        while bytes.len() < header_end + length {
            let mut chunk = [0;4096]; let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0); bytes.extend_from_slice(&chunk[..count]);
        }
        (method,path,headers,bytes[header_end..header_end+length].to_vec())
    }

    async fn reply(socket: &mut tokio::net::TcpStream, body: &[u8]) {
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
        socket.write_all(body).await.unwrap();
    }

    #[tokio::test]
    async fn remote_roundtrip_signs_exact_ciphertext_and_restores_portably() {
        let identity = liberte_shared::identity::Identity::generate();
        let expected_key = identity.public_key_bytes();
        let payload = liberte_store::BackupPayload { created_at: chrono::Utc::now().to_rfc3339(), version: "test".into(),
            channels: Vec::new(), messages: Vec::new(), channel_keys: Default::default(), servers: Vec::new(), reactions: Vec::new(),
            users: vec![liberte_store::User { pubkey: expected_key, display_name: Some("private profile name".into()), avatar_hash: None, bio: None, status: "online".into(), created_at: chrono::Utc::now() }] };
        let encrypted = backup_crypto::encrypt_identity(&serde_json::to_vec(&payload).unwrap(), BackupKind::Backup, &identity).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}",listener.local_addr().unwrap());
        let mock = tokio::spawn(async move {
            let mut stored = String::new();
            for step in 0..2 {
                let (mut socket,_) = listener.accept().await.unwrap();
                let (method,path,headers,body) = tokio::time::timeout(std::time::Duration::from_secs(5), receive_request(&mut socket)).await.unwrap();
                let auth = liberte_shared::http_auth::SignedRequest { public_key: headers["x-liberte-key"].clone(),
                    timestamp: headers["x-liberte-timestamp"].parse().unwrap(), nonce: headers["x-liberte-nonce"].clone(), signature: headers["x-liberte-signature"].clone() };
                assert_eq!(auth.verify(&method,&path,&body,chrono::Utc::now().timestamp()).unwrap(), expected_key);
                if step == 0 {
                    assert_eq!((method.as_str(),path.as_str()),("POST","/backup/sync"));
                    let request: BackupSyncRequest = serde_json::from_slice(&body).unwrap();
                    assert_eq!(request.user_pubkey_hex,hex::encode(expected_key));
                    assert!(!request.encrypted_data.contains("private profile name"));
                    stored = request.encrypted_data;
                    reply(&mut socket,serde_json::json!({"stored":true,"size_bytes":stored.len()}).to_string().as_bytes()).await;
                } else {
                    assert_eq!(method,"GET"); assert_eq!(path,format!("/backup/{}",hex::encode(expected_key))); assert!(body.is_empty());
                    reply(&mut socket,stored.as_bytes()).await;
                }
            }
        });
        send_remote_backup(&base,&identity,None,encrypted).await.unwrap();
        let restored = fetch_remote_backup(&base,&identity).await.unwrap();
        assert_eq!(restored.users,payload.users);
        mock.await.unwrap();
    }

    #[tokio::test]
    async fn remote_restore_rejects_plaintext_even_from_a_successful_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}",listener.local_addr().unwrap());
        let mock = tokio::spawn(async move {
            let (mut socket,_) = listener.accept().await.unwrap();
            receive_request(&mut socket).await;
            reply(&mut socket,b"{}").await;
        });
        assert!(fetch_remote_backup(&base,&liberte_shared::identity::Identity::generate()).await.is_err());
        mock.await.unwrap();
    }
}
