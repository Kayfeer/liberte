use thiserror::Error;

/// Errors produced by the store layer.
#[derive(Error, Debug)]
pub enum StoreError {
    /// SQLite / SQLCipher error.
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Failed to determine a platform data directory.
    #[error("Could not determine application data directory")]
    NoDataDir,

    /// Generic I/O error (e.g. creating the database directory).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A query expected exactly one row but found none.
    #[error("Record not found")]
    NotFound,

    /// Migration failure.
    #[error("Migration error: {0}")]
    Migration(String),

    /// UUID parsing error.
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    /// Hex decoding error.
    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),

    /// Chrono parsing error.
    #[error("Timestamp parse error: {0}")]
    ChronoParse(#[from] chrono::ParseError),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, StoreError>;
