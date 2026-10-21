use std::{io::{Read, Write}, path::Path, sync::{Arc, Mutex}, time::Instant};
use chrono::Utc;
use liberte_shared::{constants::MAX_FILE_SIZE, protocol::{FileAcceptance, FileOffer, WireMessage}, types::ChannelId};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tempfile::NamedTempFile;
use uuid::Uuid;
use crate::{file_transfer::{self, FileTransferDto, TransferRecord}, state::AppState};

static STAGING_LIMIT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSendResult { pub file_id: String, pub file_name: String, pub file_size: u64 }

fn stage_file(path: &Path) -> Result<(NamedTempFile, u64, [u8; 32]), String> {
    let mut source = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let metadata = source.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_SIZE as u64 { return Err("Fichier invalide ou supérieur à 50 Mio".into()); }
    let mut staged = NamedTempFile::new().map_err(|e| e.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; file_transfer::CHUNK_SIZE];
    let mut size = 0_u64;
    loop {
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        size += n as u64;
        if size > MAX_FILE_SIZE as u64 { return Err("Fichier supérieur à 50 Mio".into()); }
        hasher.update(&buffer[..n]); staged.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
    staged.flush().map_err(|e| e.to_string())?;
    Ok((staged, size, *hasher.finalize().as_bytes()))
}

#[tauri::command]
pub async fn send_file(app: AppHandle, state: State<'_, Arc<Mutex<AppState>>>, channel_id: String, file_path: String) -> Result<FileSendResult, String> {
    let _permit = STAGING_LIMIT.acquire().await.map_err(|e| e.to_string())?;
    let channel = Uuid::parse_str(&channel_id).map_err(|e| e.to_string())?;
    let path = Path::new(&file_path);
    let name = path.file_name().and_then(|v| v.to_str()).ok_or("Nom de fichier invalide")?.to_owned();
    if name.len() > 255 || name.chars().any(char::is_control) { return Err("Nom de fichier invalide".into()); }
    { let guard = state.lock().map_err(|e| e.to_string())?; file_transfer::context(&guard, channel)?; }
    let (file, size, hash) = tokio::task::spawn_blocking(move || stage_file(Path::new(&file_path))).await.map_err(|e| e.to_string())??;
    let id = Uuid::new_v4();
    let offer = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        file_transfer::reserve_capacity(&mut guard, size)?;
        let (identity, _, _) = file_transfer::context(&guard, channel)?;
        let dto = FileTransferDto { file_id: id.to_string(), channel_id: channel.to_string(), file_name: name.clone(), file_size: size, status: "offered".into(), progress: 0.0, sender_id: hex::encode(identity.public_key_bytes()), error: None };
        let record = TransferRecord::new(dto, hash, Some(file));
        file_transfer::emit(&app, &record); guard.file_transfers.insert(id, record);
        WireMessage::FileOffer(FileOffer { sender: identity.user_id(), channel_id: ChannelId(channel), file_id: id, file_name: name.clone(), file_size: size, file_hash: hash, timestamp: Utc::now() })
    };
    if let Err(error) = file_transfer::publish(state.inner(), channel, offer).await { file_transfer::fail(&app, state.inner(), id, error.clone()); return Err(error); }
    Ok(FileSendResult { file_id: id.to_string(), file_name: name, file_size: size })
}

#[tauri::command]
pub fn list_channel_files(state: State<'_, Arc<Mutex<AppState>>>, channel_id: String) -> Result<Vec<FileTransferDto>, String> {
    let channel = Uuid::parse_str(&channel_id).map_err(|e| e.to_string())?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.database.as_ref().ok_or("Base indisponible")?.get_channel_key(channel).map_err(|e| e.to_string())?;
    let _ = file_transfer::reserve_capacity(&mut guard, 0);
    let mut result: Vec<_> = guard.file_transfers.values().filter(|r| r.dto.channel_id == channel_id).map(|r| r.dto.clone()).collect();
    result.sort_by(|a, b| a.file_name.cmp(&b.file_name).then(a.file_id.cmp(&b.file_id)));
    Ok(result)
}

#[tauri::command]
pub async fn accept_file(app: AppHandle, state: State<'_, Arc<Mutex<AppState>>>, file_id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;
    let (channel, accept) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let record = guard.file_transfers.get(&id).ok_or("Offre introuvable")?;
        let size = record.dto.file_size;
        let channel = Uuid::parse_str(&record.dto.channel_id).map_err(|e| e.to_string())?;
        let (identity, _, _) = file_transfer::context(&guard, channel)?;
        if record.dto.sender_id == hex::encode(identity.public_key_bytes()) || record.dto.status != "offered" { return Err("Offre non disponible".into()); }
        file_transfer::reserve_capacity(&mut guard, size)?;
        let record = guard.file_transfers.get_mut(&id).ok_or("Offre expirée")?;
        if record.dto.status != "offered" { return Err("Offre expirée".into()); }
        let file = NamedTempFile::new().map_err(|e| e.to_string())?;
        file.as_file().set_len(size).map_err(|e| e.to_string())?;
        record.file = Some(file); record.dto.status = "receiving".into(); record.updated = Instant::now();
        file_transfer::emit(&app, record);
        (channel, WireMessage::FileAccept(FileAcceptance { file_id: id, accepter: identity.user_id() }))
    };
    if let Err(error) = file_transfer::publish(state.inner(), channel, accept).await { file_transfer::fail(&app, state.inner(), id, error.clone()); return Err(error); }
    Ok(())
}

#[tauri::command]
pub fn save_received_file(state: State<'_, Arc<Mutex<AppState>>>, file_id: String, file_path: String) -> Result<(), String> {
    let id = Uuid::parse_str(&file_id).map_err(|e| e.to_string())?;
    let guard = state.lock().map_err(|e| e.to_string())?;
    let record = guard.file_transfers.get(&id).ok_or("Fichier introuvable")?;
    if record.dto.status != "received" { return Err("Réception non terminée".into()); }
    let source = record.file.as_ref().ok_or("Fichier temporaire absent")?;
    let destination = Path::new(&file_path);
    let parent = destination.parent().filter(|p| !p.as_os_str().is_empty()).ok_or("Destination invalide")?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    std::io::copy(&mut source.reopen().map_err(|e| e.to_string())?, &mut temp).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Deserialize)]
struct BlobUploadResponse { #[serde(rename = "id", alias = "blob_id")] blob_id: String }

#[tauri::command]
pub async fn upload_premium_blob(state: State<'_, Arc<Mutex<AppState>>>, file_path: String, channel_key_hex: String) -> Result<String, String> {
    let _permit = STAGING_LIMIT.acquire().await.map_err(|e| e.to_string())?;
    let key: [u8; 32] = hex::decode(channel_key_hex).map_err(|e| e.to_string())?.try_into().map_err(|_| "Clé de canal invalide")?;
    let (server, identity, token) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        (guard.server_url.clone(), guard.identity.clone().ok_or("Aucune identité chargée")?, guard.premium_token.clone())
    };
    let encrypted = tokio::task::spawn_blocking(move || {
        let (file, _, _) = stage_file(Path::new(&file_path))?;
        if file.as_file().metadata().map_err(|e| e.to_string())?.len() > (MAX_FILE_SIZE - 40) as u64 { return Err("Le fichier chiffré dépasse la limite serveur de 50 Mio".into()); }
        let data = std::fs::read(file.path()).map_err(|e| e.to_string())?;
        liberte_shared::crypto::encrypt(&key, &data).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())??;
    let response = crate::server_client::signed_request(&server, reqwest::Method::POST, "/blob/upload", encrypted, &identity, token.as_ref(), "application/octet-stream").await?;
    let uploaded: BlobUploadResponse = response.json().await.map_err(|e| format!("Réponse serveur invalide : {e}"))?;
    Uuid::parse_str(&uploaded.blob_id).map_err(|_| "Identifiant serveur invalide")?;
    Ok(uploaded.blob_id)
}
