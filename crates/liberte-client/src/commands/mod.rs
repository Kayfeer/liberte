//! Tauri invoke command handlers.
//!
//! Each sub-module groups related commands by domain.  All public functions
//! in these modules are annotated with `#[tauri::command]` and registered
//! in the [`tauri::Builder`] invoke handler in `lib.rs`.

pub mod files;
pub mod identity;
pub mod media;
pub mod messaging;
pub mod network;
pub mod premium;
pub mod settings;
