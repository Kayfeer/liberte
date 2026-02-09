use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Could not determine application data directory")]
    NoDataDir,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Record not found")]
    NotFound,

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("Timestamp parse error: {0}")]
    ChronoParse(#[from] chrono::ParseError),
}

pub type Result<T> = std::result::Result<T, StoreError>;
