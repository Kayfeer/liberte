//! # liberte-client
//!
//! Tauri v2 desktop client for the Liberte encrypted communication platform.
//!
//! This crate wires together the Tauri application shell with the shared
//! crypto primitives (`liberte-shared`), the P2P networking layer
//! (`liberte-net`), the local encrypted database (`liberte-store`), and
//! the media engine (`liberte-media`).
//!
//! All interaction between the TypeScript frontend and Rust happens through
//! Tauri invoke commands defined in [`commands`] and events defined in
//! [`events`].

pub mod commands;
pub mod events;
pub mod state;

use std::sync::{Arc, Mutex};

use tracing_subscriber::{fmt, EnvFilter};

use crate::state::AppState;

/// Build and run the Tauri application.
///
/// This is the sole public entry point called from `main.rs`.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to initialise (e.g. missing WebView).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialise tracing with an env-filter that respects `RUST_LOG`.
    // Default level is `info` for our crates, `warn` for everything else.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("liberte_client_lib=debug,liberte_net=debug,liberte_store=info,liberte_media=info,warn")
    });

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    tracing::info!("Starting Liberte desktop client");

    // Shared application state
    let app_state = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        // -- Plugins --
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        // -- Managed state --
        .manage(app_state)
        // -- Invoke handlers --
        .invoke_handler(tauri::generate_handler![
            // Identity
            commands::identity::create_identity,
            commands::identity::load_identity,
            commands::identity::export_pubkey,
            // Network
            commands::network::connect_peer,
            commands::network::list_peers,
            commands::network::get_connection_mode,
            // Messaging
            commands::messaging::send_message,
            commands::messaging::get_messages,
            commands::messaging::list_channels,
            // Media / calls
            commands::media::start_call,
            commands::media::end_call,
            commands::media::toggle_mute,
            commands::media::toggle_video,
            // Files
            commands::files::send_file,
            commands::files::upload_premium_blob,
            // Premium
            commands::premium::check_premium,
            commands::premium::activate_premium,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_server_info,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Tauri application");
}
