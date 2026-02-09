//! # liberte-store
//!
//! Local encrypted storage for the Liberte application, backed by SQLCipher.
//!
//! All data at rest is encrypted with a 256-bit key derived from the user's
//! identity.  The crate exposes a synchronous `Database` handle that wraps a
//! `rusqlite::Connection` and provides typed CRUD helpers for every domain
//! model.

pub mod blobs;
pub mod channels;
pub mod database;
pub mod messages;
pub mod migrations;
pub mod models;
pub mod servers;

mod error;

pub use database::Database;
pub use error::StoreError;
pub use models::*;
