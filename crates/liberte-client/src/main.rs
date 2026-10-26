// Prevent a console window from appearing on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg_attr(target_os = "linux", tauri_runtime_cef::cef_entry_point)]
fn main() {
    liberte_client_lib::run();
}
