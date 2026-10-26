use std::sync::{Arc, Mutex};

use rusqlite::OptionalExtension;
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

fn ensure_settings_table(db: &liberte_store::Database) -> Result<(), String> {
    db.conn()
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );",
        )
        .map_err(|e| format!("Cannot open settings: {e}"))
}

pub(crate) fn read_settings(db: &liberte_store::Database) -> Result<AppSettings, String> {
    ensure_settings_table(db)?;
    let json: Option<String> = db
        .conn()
        .query_row("SELECT json FROM app_settings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    match json {
        Some(json) => {
            serde_json::from_str(&json).map_err(|e| format!("Corrupt settings JSON: {e}"))
        }
        None => Ok(AppSettings::default()),
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<Mutex<AppState>>>) -> Result<AppSettings, String> {
    let guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    match &guard.database {
        Some(db) => read_settings(db),
        None => Ok(AppSettings::default()),
    }
}

/// Caller keeps the state lock across both persistence and activation, so concurrent
/// settings updates cannot leave the live server origin behind the committed value.
fn persist_settings(state: &mut AppState, mut settings: AppSettings) -> Result<(), String> {
    settings.server_url = settings.server_url.trim().to_owned();
    if !settings.server_url.is_empty() {
        crate::server_client::endpoint(&settings.server_url, "/info")?;
    }
    if settings
        .display_name
        .as_ref()
        .is_some_and(|name| name.len() > 128)
    {
        return Err("Nom trop long (128 octets maximum)".into());
    }
    let db = state.database.as_ref().ok_or("Database not opened")?;
    ensure_settings_table(db)?;
    let json = serde_json::to_string(&settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    db.conn().execute(
        "INSERT INTO app_settings (id, json) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        rusqlite::params![json],
    ).map_err(|e| format!("Failed to save settings: {e}"))?;

    state.server_url = settings.server_url;
    crate::commands::premium::restore_status(state);
    info!("Settings updated");
    Ok(())
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: AppSettings,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
    persist_settings(&mut guard, settings)
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
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Impossible de contacter le serveur: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Le serveur a répondu {}", resp.status()));
    }

    crate::server_client::read_json_limited(resp, 65536).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state(path: &std::path::Path) -> AppState {
        let mut state = AppState::new();
        state.database = Some(liberte_store::Database::open_at(path, &[0; 32]).unwrap());
        state
    }

    #[test]
    fn settings_update_keeps_persisted_and_live_server_together_on_success_and_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.db");
        let mut state = test_state(&path);
        persist_settings(
            &mut state,
            AppSettings {
                server_url: " https://one.example.test ".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(state.server_url, "https://one.example.test");
        assert_eq!(
            read_settings(state.database.as_ref().unwrap())
                .unwrap()
                .server_url,
            state.server_url
        );
        state.database.as_ref().unwrap().conn().execute_batch(
            "CREATE TRIGGER reject_settings BEFORE UPDATE ON app_settings BEGIN SELECT RAISE(ABORT,'write failed'); END;"
        ).unwrap();
        assert!(persist_settings(
            &mut state,
            AppSettings {
                server_url: "https://two.example.test".into(),
                ..Default::default()
            }
        )
        .is_err());
        assert_eq!(state.server_url, "https://one.example.test");
        assert_eq!(
            read_settings(state.database.as_ref().unwrap())
                .unwrap()
                .server_url,
            state.server_url
        );
        drop(state);
        let reopened = test_state(&path);
        assert_eq!(
            read_settings(reopened.database.as_ref().unwrap())
                .unwrap()
                .server_url,
            "https://one.example.test"
        );
    }

    #[test]
    fn malformed_settings_are_errors_instead_of_silent_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(&dir.path().join("corrupt.db"));
        let db = state.database.as_ref().unwrap();
        assert!(read_settings(db).unwrap().server_url.is_empty());
        db.conn()
            .execute("INSERT INTO app_settings VALUES(1,'broken json')", [])
            .unwrap();
        assert!(read_settings(db).is_err());
        db.conn().execute_batch("DROP TABLE app_settings; CREATE TABLE app_settings (id INTEGER PRIMARY KEY, wrong_column TEXT);").unwrap();
        assert!(read_settings(db).is_err());
    }
}
