use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::info;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub display_name: Option<String>,
    pub notifications_enabled: bool,
    pub start_minimised: bool,
    pub audio_input_device: String,
    pub audio_output_device: String,
    pub auto_connect: bool,
    pub theme: String,
    pub server_url: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            display_name: None,
            notifications_enabled: true,
            start_minimised: false,
            audio_input_device: "default".into(),
            audio_output_device: "default".into(),
            auto_connect: true,
            theme: "dark".into(),
            server_url: String::new(),
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<Mutex<AppState>>>) -> Result<AppSettings, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let Some(ref db) = guard.database else {
        return Ok(AppSettings::default());
    };

    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );",
    );

    let result: Result<String, _> =
        db.conn()
            .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
                row.get(0)
            });

    match result {
        Ok(json) => {
            let settings: AppSettings =
                serde_json::from_str(&json).map_err(|e| format!("Corrupt settings JSON: {e}"))?;
            Ok(settings)
        }
        Err(_) => Ok(AppSettings::default()),
    }
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<(), String> {
    if !settings.server_url.trim().is_empty() {
        crate::server_client::endpoint(&settings.server_url, "/info")?;
    }
    if settings.display_name.as_ref().is_some_and(|name| name.len() > 128) {
        return Err("Nom trop long (128 octets maximum)".into());
    }
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );",
    );

    let json = serde_json::to_string(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    db.conn()
        .execute(
            "INSERT OR REPLACE INTO app_settings (id, json) VALUES (1, ?1)",
            rusqlite::params![json],
        )
        .map_err(|e| format!("Failed to save settings: {e}"))?;

    info!("Settings updated");

    // sync server_url to live state — drop first guard before re-locking
    drop(guard);
    if let Ok(mut guard) = state.lock() {
        guard.server_url = settings.server_url.clone();
        crate::commands::premium::restore_status(&mut guard);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    #[serde(alias = "premium_required")]
    pub premium_required: bool,
    #[serde(alias = "registration_open")]
    pub registration_open: bool,
    #[serde(alias = "max_peers")]
    pub max_peers: usize,
    #[serde(default, alias = "payment_server_pubkey")]
    pub payment_server_pubkey: Option<String>,
}

#[tauri::command]
pub async fn get_server_info(server_url: String) -> Result<ServerInfo, String> {
    let url = crate::server_client::endpoint(&server_url, "/info")?;

    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build().map_err(|e| e.to_string())?.get(url).send()
        .await
        .map_err(|e| format!("Impossible de contacter le serveur: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Le serveur a répondu {}", resp.status()));
    }

    crate::server_client::read_json_limited(resp, 65536).await
}

