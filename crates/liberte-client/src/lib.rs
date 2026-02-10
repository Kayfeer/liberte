pub mod commands;
pub mod events;
pub mod state;
pub mod swarm_bridge;

use std::sync::{Arc, Mutex};

use tracing_subscriber::{fmt, EnvFilter};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "liberte_client_lib=debug,liberte_net=debug,liberte_store=info,liberte_media=info,warn",
        )
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Ok(mut guard) = app_state.lock() {
                guard.app_handle = Some(handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::identity::create_identity,
            commands::identity::load_identity,
            commands::identity::export_pubkey,
            commands::identity::set_display_name,
            commands::identity::set_bio,
            commands::identity::set_status,
            commands::network::connect_peer,
            commands::network::list_peers,
            commands::network::get_connection_mode,
            commands::messaging::send_message,
            commands::messaging::get_messages,
            commands::messaging::list_channels,
            commands::messaging::search_messages,
            commands::messaging::add_reaction,
            commands::messaging::remove_reaction,
            commands::messaging::get_reactions,
            commands::media::start_call,
            commands::media::end_call,
            commands::media::toggle_mute,
            commands::media::toggle_video,
            commands::media::set_call_mode,
            commands::media::get_call_state,
            commands::files::send_file,
            commands::files::upload_premium_blob,
            commands::premium::check_premium,
            commands::premium::activate_premium,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_server_info,
            commands::channels::create_channel,
            commands::channels::generate_invite,
            commands::channels::accept_invite,
            commands::channels::get_all_channel_keys,
            commands::backup::export_backup,
            commands::backup::save_backup_to_file,
            commands::backup::auto_backup,
            commands::backup::import_backup,
            commands::backup::list_backups,
            commands::profile::export_profile,
            commands::profile::import_profile,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Tauri application");
}
