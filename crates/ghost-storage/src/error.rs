//! Top-level error type for ghost-storage.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("schema migration failed at version {version}: {detail}")]
    Migration { version: u32, detail: String },

    #[error("schema downgrade attempted: db is at v{db_version}, app supports v{app_version}")]
    SchemaTooNew { db_version: u32, app_version: u32 },

    #[error("invalid blob in {table}.{column}: {detail}")]
    InvalidBlob {
        table: &'static str,
        column: &'static str,
        detail: String,
    },

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
