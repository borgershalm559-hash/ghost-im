//! Top-level error type for ghost-server.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("network: {0}")]
    Network(#[from] ghost_network::NetworkError),

    #[error("storage: {0}")]
    Storage(#[from] ghost_storage::StorageError),

    #[error("server reported error: {0}")]
    Remote(String),

    #[error("no key packages available")]
    NoKeyPackagesAvailable,

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("entity not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, ServerError>;
