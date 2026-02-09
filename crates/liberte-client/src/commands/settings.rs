//! User settings Tauri commands.
//!
//! Provides get/update commands for application settings.  Settings are
//! stored as a JSON blob in the local encrypted database.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::info;

use crate::state::AppState;

/// Application settings visible to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// User-chosen display name.
    pub display_name: Option<String>,
    /// Whether desktop notifications are enabled.
    pub notifications_enabled: bool,
    /// Whether to start minimised to tray on launch.
    pub start_minimised: bool,
    /// Audio input device name (or "default").
    pub audio_input_device: String,
    /// Audio output device name (or "default").
    pub audio_output_device: String,
    /// Whether to auto-connect to the swarm on startup.
    pub auto_connect: bool,
    /// UI theme preference ("dark" / "light" / "system").
    pub theme: String,
    /// URL of the relay server (self-hosted or managed).
    /// Empty string means no server configured.
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

/// Load the current application settings from the database.
///
/// If no settings row exists yet, returns the defaults.
#[tauri::command]
pub fn get_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<AppSettings, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let Some(ref db) = guard.database else {
        // No database open yet; return defaults.
        return Ok(AppSettings::default());
    };

    // Ensure the settings table exists
    let _ = db.conn().execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );",
    );

    let result: Result<String, _> = db.conn().query_row(
        "SELECT json FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    );

    match result {
        Ok(json) => {
            let settings: AppSettings = serde_json::from_str(&json)
                .map_err(|e| format!("Corrupt settings JSON: {e}"))?;
            Ok(settings)
        }
        Err(_) => Ok(AppSettings::default()),
    }
}

/// Persist updated application settings.
///
/// The entire settings object is replaced atomically.
#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<(), String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;

    let db = guard
        .database
        .as_ref()
        .ok_or_else(|| "Database not opened".to_string())?;

    // Ensure the settings table exists
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

    // Update the server_url in the live app state.
    if let Ok(mut guard) = state.lock() {
        guard.server_url = settings.server_url.clone();
    }

    Ok(())
}

/// Info returned by a remote Liberté server at GET /info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub premium_required: bool,
    pub registration_open: bool,
    pub max_peers: usize,
}

/// Query a Liberté server's public info.
///
/// The frontend calls this when the user enters a server URL to verify
/// it is a valid Liberté instance before saving.
#[tauri::command]
pub async fn get_server_info(server_url: String) -> Result<ServerInfo, String> {
    let url = format!("{}/info", server_url.trim_end_matches('/'));

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Impossible de contacter le serveur: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Le serveur a répondu {}", resp.status()));
    }

    let info: ServerInfo = resp
        .json()
        .await
        .map_err(|e| format!("Réponse invalide du serveur: {e}"))?;

    Ok(info)
}
