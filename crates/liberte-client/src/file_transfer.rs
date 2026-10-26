//! Explicit-consent, authenticated channel file transfers. Files are bounded,
//! checksummed and held in private temporary files until the user saves them.
use crate::state::AppState;
use liberte_shared::{
    constants::MAX_FILE_SIZE,
    identity::Identity,
    protocol::{FileChunk, FileCompletion, WireMessage},
    types::{ChannelId, UserId},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tempfile::NamedTempFile;
use uuid::Uuid;

pub const CHUNK_SIZE: usize = 32 * 1024;
const MAX_RECORDS: usize = 256;
const MAX_STAGED_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferDto {
    pub file_id: String,
    pub channel_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub status: String,
    pub progress: f64,
    pub sender_id: String,
    pub error: Option<String>,
}

pub struct TransferRecord {
    pub dto: FileTransferDto,
    pub hash: [u8; 32],
    pub file: Option<NamedTempFile>,
    pub received: HashSet<u32>,
    pub recipients: HashSet<[u8; 32]>,
    recipient_attempts: HashMap<[u8; 32], u8>,
    pub acknowledgements: HashSet<[u8; 32]>,
    pub sending: bool,
    pub attempts: u8,
    pub updated: Instant,
}

impl TransferRecord {
    pub fn new(dto: FileTransferDto, hash: [u8; 32], file: Option<NamedTempFile>) -> Self {
        Self {
            dto,
            hash,
            file,
            received: HashSet::new(),
            recipients: HashSet::new(),
            recipient_attempts: HashMap::new(),
            acknowledgements: HashSet::new(),
            sending: false,
            attempts: 0,
            updated: Instant::now(),
        }
    }
}

pub fn emit(app: &AppHandle, record: &TransferRecord) {
    let _ = app.emit("file-transfer-updated", &record.dto);
}

pub fn reserve_capacity(
    state: &mut AppState,
    bytes: u64,
    adding_record: bool,
) -> Result<(), String> {
    // Offers expire and partial files are deleted even if a sender disappears.
    for record in state.file_transfers.values_mut() {
        if record.updated.elapsed() > Duration::from_secs(180)
            && matches!(
                record.dto.status.as_str(),
                "receiving" | "sending" | "offered" | "failed"
            )
        {
            record.dto.status = "failed".into();
            record.dto.error = Some("Transfert expiré. Demandez un nouvel envoi.".into());
            record.file = None;
        }
    }
    state
        .file_transfers
        .retain(|_, r| r.updated.elapsed() < Duration::from_secs(86_400));
    if adding_record && state.file_transfers.len() >= MAX_RECORDS {
        return Err("Trop de transferts en attente".into());
    }
    let staged: u64 = state
        .file_transfers
        .values()
        .filter(|r| r.file.is_some())
        .map(|r| r.dto.file_size)
        .sum();
    if staged.saturating_add(bytes) > MAX_STAGED_BYTES {
        return Err("Limite de fichiers temporaires atteinte (200 Mio)".into());
    }
    Ok(())
}

pub fn context(
    state: &AppState,
    channel: Uuid,
) -> Result<
    (
        Identity,
        [u8; 32],
        tokio::sync::mpsc::Sender<liberte_net::SwarmCommand>,
    ),
    String,
> {
    let identity = state.identity.clone().ok_or("Aucune identité chargée")?;
    let key = state
        .database
        .as_ref()
        .ok_or("Base indisponible")?
        .get_channel_key(channel)
        .map_err(|e| e.to_string())?;
    let key: [u8; 32] = hex::decode(key)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "Clé de canal invalide")?;
    let tx = state.swarm_cmd_tx.clone().ok_or("Réseau non démarré")?;
    Ok((identity, key, tx))
}

pub async fn publish(
    state: &Arc<Mutex<AppState>>,
    channel: Uuid,
    msg: WireMessage,
) -> Result<(), String> {
    let (identity, key, tx) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        context(&guard, channel)?
    };
    let channel = ChannelId(channel);
    let data = msg.seal(channel.clone(), &key, &identity)?;
    liberte_net::publish_confirmed(&tx, channel.to_topic(), data).await
}

pub fn fail(app: &AppHandle, state: &Arc<Mutex<AppState>>, id: Uuid, error: String) {
    if let Ok(mut guard) = state.lock() {
        let own = guard
            .identity
            .as_ref()
            .map(|id| hex::encode(id.public_key_bytes()));
        if let Some(r) = guard.file_transfers.get_mut(&id) {
            r.dto.status = "failed".into();
            r.dto.error = Some(error);
            r.sending = false;
            r.updated = Instant::now();
            if own.as_deref() != Some(r.dto.sender_id.as_str()) {
                r.file = None;
            }
            emit(app, r);
        }
    }
}

pub fn chunk_count(size: u64) -> u32 {
    size.div_ceil(CHUNK_SIZE as u64).max(1) as u32
}

fn receive_chunk(record: &mut TransferRecord, chunk: &FileChunk) -> Result<bool, String> {
    let count = chunk_count(record.dto.file_size);
    if chunk.total_chunks != count || chunk.chunk_index >= count {
        return Err("Nombre de blocs invalide".into());
    }
    let offset = u64::from(chunk.chunk_index) * CHUNK_SIZE as u64;
    let expected = (record.dto.file_size.saturating_sub(offset)).min(CHUNK_SIZE as u64) as usize;
    if chunk.data.len() != expected {
        return Err("Taille de bloc invalide".into());
    }
    let file = record.file.as_mut().ok_or("Fichier temporaire absent")?;
    if record.received.insert(chunk.chunk_index) {
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        file.write_all(&chunk.data).map_err(|e| e.to_string())?;
    }
    record.updated = Instant::now();
    record.dto.progress = record.received.len() as f64 / f64::from(count) * 100.0;
    if record.received.len() != count as usize {
        return Ok(false);
    }
    file.flush().map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    if hasher.finalize().as_bytes() != &record.hash {
        return Err("Intégrité du fichier invalide".into());
    }
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    record.dto.status = "received".into();
    Ok(true)
}

/// Count explicit retries per receiver, not across the whole channel.
/// A new receiver can join the current stream without spawning another sender.
fn accept_recipient(record: &mut TransferRecord, recipient: [u8; 32]) -> Option<bool> {
    let known = record.recipients.contains(&recipient);
    if (!known && record.recipients.len() >= 64)
        || record.acknowledgements.contains(&recipient)
        || (known && (record.sending || record.updated.elapsed() < Duration::from_secs(5)))
    {
        return None;
    }
    let attempts = record.recipient_attempts.entry(recipient).or_default();
    if *attempts >= 3 {
        return None;
    }
    *attempts += 1;
    record.recipients.insert(recipient);
    let start = !record.sending;
    record.sending = true;
    record.dto.status = "sending".into();
    record.dto.error = None;
    record.updated = Instant::now();
    Some(start)
}

pub fn handle_incoming(
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    channel: ChannelId,
    author: UserId,
    message: WireMessage,
) {
    let mut start = None;
    let mut completion = None;
    {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        let Some(own) = guard.identity.as_ref().map(Identity::user_id) else {
            return;
        };
        if author == own {
            return;
        }
        match message {
            WireMessage::FileOffer(offer) => {
                if offer.sender != author
                    || offer.channel_id != channel
                    || offer.file_size > MAX_FILE_SIZE as u64
                    || offer.file_name.is_empty()
                    || offer.file_name.len() > 255
                    || offer
                        .file_name
                        .chars()
                        .any(|c| c.is_control() || c == '/' || c == '\\')
                {
                    return;
                }
                if guard.file_transfers.contains_key(&offer.file_id)
                    || reserve_capacity(&mut guard, 0, true).is_err()
                {
                    return;
                }
                let dto = FileTransferDto {
                    file_id: offer.file_id.to_string(),
                    channel_id: channel.0.to_string(),
                    file_name: offer.file_name,
                    file_size: offer.file_size,
                    status: "offered".into(),
                    progress: 0.0,
                    sender_id: hex::encode(author.0),
                    error: None,
                };
                let record = TransferRecord::new(dto, offer.file_hash, None);
                emit(app, &record);
                guard.file_transfers.insert(offer.file_id, record);
            }
            WireMessage::FileAccept(accept) => {
                if accept.accepter != author {
                    return;
                }
                let Some(record) = guard.file_transfers.get_mut(&accept.file_id) else {
                    return;
                };
                if record.dto.channel_id != channel.0.to_string()
                    || record.dto.sender_id != hex::encode(own.0)
                    || record.file.is_none()
                {
                    return;
                }
                if let Some(start_sending) = accept_recipient(record, author.0) {
                    if start_sending {
                        start = Some(accept.file_id);
                    }
                    emit(app, record);
                }
            }
            WireMessage::FileChunk(chunk) => {
                let Some(record) = guard.file_transfers.get_mut(&chunk.file_id) else {
                    return;
                };
                if record.dto.channel_id != channel.0.to_string()
                    || record.dto.sender_id != hex::encode(author.0)
                    || !matches!(record.dto.status.as_str(), "receiving" | "received")
                {
                    return;
                }
                if record.dto.status == "received"
                    && chunk.chunk_index + 1 == chunk_count(record.dto.file_size)
                {
                    completion = Some((chunk.file_id, record.hash, own));
                } else if record.dto.status == "receiving" {
                    match receive_chunk(record, &chunk) {
                        Ok(true) => completion = Some((chunk.file_id, record.hash, own)),
                        Ok(false) => {}
                        Err(error) => {
                            record.dto.status = "failed".into();
                            record.dto.error = Some(error);
                            record.file = None;
                        }
                    }
                    emit(app, record);
                }
            }
            WireMessage::FileComplete(done) => {
                let Some(record) = guard.file_transfers.get_mut(&done.file_id) else {
                    return;
                };
                if done.receiver != author
                    || done.channel_id != channel
                    || record.dto.channel_id != channel.0.to_string()
                    || !record.recipients.contains(&author.0)
                    || done.file_hash != record.hash
                {
                    return;
                }
                record.acknowledgements.insert(author.0);
                if record.recipients.is_subset(&record.acknowledgements) {
                    record.dto.status = "completed".into();
                    record.dto.progress = 100.0;
                }
                record.updated = Instant::now();
                emit(app, record);
            }
            _ => {}
        }
    }
    if let Some(id) = start {
        let state = state.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = send_chunks(&app, &state, channel.0, id).await {
                fail(&app, &state, id, error);
            }
        });
    }
    if let Some((id, hash, receiver)) = completion {
        let state = state.clone();
        tauri::async_runtime::spawn(async move {
            let _ = publish(
                &state,
                channel.0,
                WireMessage::FileComplete(FileCompletion {
                    file_id: id,
                    receiver,
                    channel_id: channel,
                    file_hash: hash,
                }),
            )
            .await;
        });
    }
}

pub fn start_cleanup(state: std::sync::Weak<Mutex<AppState>>, app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let Some(state) = state.upgrade() else {
                break;
            };
            let Ok(mut guard) = state.lock() else {
                break;
            };
            let expired: Vec<_> = guard
                .file_transfers
                .iter()
                .filter(|(_, r)| {
                    r.updated.elapsed() > Duration::from_secs(180)
                        && matches!(r.dto.status.as_str(), "receiving" | "sending" | "offered")
                })
                .map(|(id, _)| *id)
                .collect();
            let _ = reserve_capacity(&mut guard, 0, false);
            for id in expired {
                if let Some(record) = guard.file_transfers.get(&id) {
                    emit(&app, record);
                }
            }
        }
    });
}

async fn send_chunks(
    app: &AppHandle,
    state: &Arc<Mutex<AppState>>,
    channel: Uuid,
    id: Uuid,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let (file, size) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        let r = guard.file_transfers.get(&id).ok_or("Transfert absent")?;
        (
            r.file
                .as_ref()
                .ok_or("Fichier absent")?
                .reopen()
                .map_err(|e| e.to_string())?,
            r.dto.file_size,
        )
    };
    let mut file = tokio::fs::File::from_std(file);
    let total = chunk_count(size);
    // GossipSub is not a reliable stream: resend missing rounds until recipients
    // acknowledge the complete BLAKE3-verified file. Duplicate chunks are harmless.
    for _ in 0..4 {
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(|e| e.to_string())?;
        for index in 0..total {
            let length = (size.saturating_sub(u64::from(index) * CHUNK_SIZE as u64))
                .min(CHUNK_SIZE as u64) as usize;
            let mut data = vec![0; length];
            file.read_exact(&mut data)
                .await
                .map_err(|e| e.to_string())?;
            publish(
                state,
                channel,
                WireMessage::FileChunk(FileChunk {
                    file_id: id,
                    chunk_index: index,
                    total_chunks: total,
                    data,
                }),
            )
            .await?;
            {
                let mut guard = state.lock().map_err(|e| e.to_string())?;
                if let Some(r) = guard.file_transfers.get_mut(&id) {
                    r.updated = Instant::now();
                    r.dto.progress = f64::from(index + 1) / f64::from(total) * 99.0;
                    if index % 16 == 0 {
                        emit(app, r);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let r = guard
            .file_transfers
            .get_mut(&id)
            .ok_or("Transfert absent")?;
        if r.recipients.is_subset(&r.acknowledgements) {
            r.sending = false;
            r.dto.status = "completed".into();
            r.dto.progress = 100.0;
            emit(app, r);
            return Ok(());
        }
    }
    Err("Réception non confirmée après plusieurs tentatives".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(data: &[u8]) -> TransferRecord {
        let file = NamedTempFile::new().unwrap();
        file.as_file().set_len(data.len() as u64).unwrap();
        TransferRecord::new(
            FileTransferDto {
                file_id: Uuid::new_v4().to_string(),
                channel_id: Uuid::new_v4().to_string(),
                file_name: "test.bin".into(),
                file_size: data.len() as u64,
                status: "receiving".into(),
                progress: 0.0,
                sender_id: String::new(),
                error: None,
            },
            *blake3::hash(data).as_bytes(),
            Some(file),
        )
    }
    #[test]
    fn out_of_order_duplicates_and_integrity() {
        let data = vec![7; CHUNK_SIZE + 9];
        let mut r = record(&data);
        let id = Uuid::new_v4();
        let tail = FileChunk {
            file_id: id,
            chunk_index: 1,
            total_chunks: 2,
            data: data[CHUNK_SIZE..].to_vec(),
        };
        assert!(!receive_chunk(&mut r, &tail).unwrap());
        assert!(!receive_chunk(&mut r, &tail).unwrap());
        assert!(receive_chunk(
            &mut r,
            &FileChunk {
                file_id: id,
                chunk_index: 0,
                total_chunks: 2,
                data: data[..CHUNK_SIZE].to_vec()
            }
        )
        .unwrap());
        assert_eq!(r.dto.status, "received");
    }
    #[test]
    fn malformed_and_corrupt_chunks_fail() {
        let mut r = record(b"ok");
        let id = Uuid::new_v4();
        assert!(receive_chunk(
            &mut r,
            &FileChunk {
                file_id: id,
                chunk_index: 1,
                total_chunks: 1,
                data: vec![]
            }
        )
        .is_err());
        assert!(receive_chunk(
            &mut r,
            &FileChunk {
                file_id: id,
                chunk_index: 0,
                total_chunks: 1,
                data: b"no".to_vec()
            }
        )
        .is_err());
    }
    #[test]
    fn later_channel_members_do_not_exhaust_each_others_transfer_attempts() {
        let mut transfer = record(b"shared file");
        for index in 0..8_u8 {
            let recipient = [index; 32];
            assert_eq!(accept_recipient(&mut transfer, recipient), Some(true));
            transfer.acknowledgements.insert(recipient);
            transfer.sending = false;
            transfer.dto.status = "completed".into();
        }
        assert_eq!(transfer.recipients.len(), 8);
    }

    #[test]
    fn receiver_retries_remain_bounded_and_do_not_spawn_parallel_senders() {
        let mut transfer = record(b"shared file");
        let first = [1; 32];
        assert_eq!(accept_recipient(&mut transfer, first), Some(true));
        assert_eq!(accept_recipient(&mut transfer, first), None);
        assert_eq!(accept_recipient(&mut transfer, [2; 32]), Some(false));
        for _ in 0..2 {
            transfer.sending = false;
            transfer.updated = Instant::now() - Duration::from_secs(6);
            assert_eq!(accept_recipient(&mut transfer, first), Some(true));
        }
        transfer.sending = false;
        transfer.updated = Instant::now() - Duration::from_secs(6);
        assert_eq!(accept_recipient(&mut transfer, first), None);
        assert_eq!(accept_recipient(&mut transfer, [3; 32]), Some(true));
    }

    #[test]
    fn empty_file_completes() {
        let mut r = record(b"");
        assert!(receive_chunk(
            &mut r,
            &FileChunk {
                file_id: Uuid::new_v4(),
                chunk_index: 0,
                total_chunks: 1,
                data: vec![]
            }
        )
        .unwrap());
    }
}
