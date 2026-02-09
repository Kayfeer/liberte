use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::info;

use liberte_store::backup::ImportStats;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub auto_backup_enabled: bool,
    /// Interval in minutes between auto-backups
    pub interval_minutes: u32,
    /// Folder path for local backups (empty = default data dir)
    pub backup_dir: String,
    /// Whether to sync backups to the connected relay server
    pub server_sync_enabled: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            auto_backup_enabled: true,
            interval_minutes: 30,
            backup_dir: String::new(),
            server_sync_enabled: false,
        }
    }
}

/// Export the full database as a JSON string (channels + messages + keys).
#[tauri::command]
pub fn export_backup(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let payload = db
        .export_backup()
        .map_err(|e| format!("Export failed: {e}"))?;

    let json =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("Serialization failed: {e}"))?;

    info!(
        channels = payload.channels.len(),
        messages = payload.messages.len(),
        keys = payload.channel_keys.len(),
        "Backup exported"
    );

    Ok(json)
}

/// Save backup JSON to a file on disk.
#[tauri::command]
pub async fn save_backup_to_file(
    state: State<'_, Arc<Mutex<AppState>>>,
    file_path: String,
) -> Result<String, String> {
    let json = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard
            .database
            .as_ref()
            .ok_or_else(|| "Database not opened".to_string())?;

        let payload = db
            .export_backup()
            .map_err(|e| format!("Export failed: {e}"))?;

        serde_json::to_string_pretty(&payload).map_err(|e| format!("Serialization failed: {e}"))?
    };

    tokio::fs::write(&file_path, json.as_bytes())
        .await
        .map_err(|e| format!("Failed to write backup file: {e}"))?;

    info!(path = %file_path, "Backup saved to file");

    Ok(file_path)
}

/// Auto-backup: save to default backup directory. Returns the file path.
#[tauri::command]
pub async fn auto_backup(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let (json, data_dir) = {
        let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        let db = guard
            .database
            .as_ref()
            .ok_or_else(|| "Database not opened".to_string())?;

        let payload = db
            .export_backup()
            .map_err(|e| format!("Export failed: {e}"))?;

        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("Serialization failed: {e}"))?;

        let dirs = directories::ProjectDirs::from("com", "liberte", "liberte")
            .ok_or_else(|| "Cannot determine data directory".to_string())?;

        (json, dirs.data_dir().to_path_buf())
    };

    let backup_dir = data_dir.join("backups");
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|e| format!("Failed to create backup dir: {e}"))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let file_name = format!("liberte_backup_{timestamp}.json");
    let file_path = backup_dir.join(&file_name);

    tokio::fs::write(&file_path, json.as_bytes())
        .await
        .map_err(|e| format!("Failed to write backup: {e}"))?;

    // Keep only the 10 most recent backups
    cleanup_old_backups(&backup_dir, 10).await;

    let path_str = file_path.to_string_lossy().to_string();
    info!(path = %path_str, "Auto-backup completed");

    Ok(path_str)
}

/// Import a backup from JSON string. Merges with existing data.
#[tauri::command]
pub fn import_backup(
    state: State<'_, Arc<Mutex<AppState>>>,
    json: String,
) -> Result<ImportStats, String> {
    let payload: liberte_store::BackupPayload =
        serde_json::from_str(&json).map_err(|e| format!("Invalid backup JSON: {e}"))?;

    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let stats = db
        .import_backup(&payload)
        .map_err(|e| format!("Import failed: {e}"))?;

    info!(
        channels = stats.channels_imported,
        messages = stats.messages_imported,
        keys = stats.keys_imported,
        "Backup imported"
    );

    Ok(stats)
}

/// List available backup files in the default backup directory.
#[tauri::command]
pub async fn list_backups() -> Result<Vec<BackupFileInfo>, String> {
    let dirs = directories::ProjectDirs::from("com", "liberte", "liberte")
        .ok_or_else(|| "Cannot determine data directory".to_string())?;

    let backup_dir = dirs.data_dir().join("backups");
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&backup_dir)
        .await
        .map_err(|e| format!("Failed to read backup dir: {e}"))?;

    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(meta) = entry.metadata().await {
                entries.push(BackupFileInfo {
                    file_name: entry.file_name().to_string_lossy().to_string(),
                    file_path: path.to_string_lossy().to_string(),
                    size_bytes: meta.len(),
                    modified: meta
                        .modified()
                        .ok()
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Utc> = t.into();
                            dt.to_rfc3339()
                        })
                        .unwrap_or_default(),
                });
            }
        }
    }

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileInfo {
    pub file_name: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub modified: String,
}

async fn cleanup_old_backups(dir: &std::path::Path, keep: usize) {
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else {
        return;
    };

    let mut files: Vec<(String, std::time::SystemTime)> = Vec::new();
    while let Ok(Some(entry)) = rd.next_entry().await {
        if let Ok(meta) = entry.metadata().await {
            if let Ok(modified) = meta.modified() {
                files.push((entry.path().to_string_lossy().to_string(), modified));
            }
        }
    }

    files.sort_by(|a, b| b.1.cmp(&a.1));

    for file in files.iter().skip(keep) {
        let _ = tokio::fs::remove_file(&file.0).await;
    }
}
