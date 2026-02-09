pub mod commands;
pub mod events;
pub mod state;

use std::sync::{Arc, Mutex};

use tracing_subscriber::{fmt, EnvFilter};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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

    tracing::info!("Starting Liberté desktop client");

    let app_state = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::identity::create_identity,
            commands::identity::load_identity,
            commands::identity::export_pubkey,
            commands::network::connect_peer,
            commands::network::list_peers,
            commands::network::get_connection_mode,
            commands::messaging::send_message,
            commands::messaging::get_messages,
            commands::messaging::list_channels,
            commands::media::start_call,
            commands::media::end_call,
            commands::media::toggle_mute,
            commands::media::toggle_video,
            commands::files::send_file,
            commands::files::upload_premium_blob,
            commands::premium::check_premium,
            commands::premium::activate_premium,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_server_info,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Tauri application");
}
