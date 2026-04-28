//! Frontend-facing error type. Tauri commands return `Result<T, CommandError>`;
//! the frontend receives the message as a string.

use serde::Serialize;

/// String-shaped error for the JS bridge. Wraps any internal error type as a
/// flat human-readable message — the frontend never sees Rust error trees.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("{0}")]
pub struct CommandError(pub String);

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<ghost_client::ClientError> for CommandError {
    fn from(e: ghost_client::ClientError) -> Self {
        Self(format!("client: {e}"))
    }
}

impl From<ghost_identity::IdentityError> for CommandError {
    fn from(e: ghost_identity::IdentityError) -> Self {
        Self(format!("identity: {e}"))
    }
}

impl From<ghost_storage::StorageError> for CommandError {
    fn from(e: ghost_storage::StorageError) -> Self {
        Self(format!("storage: {e}"))
    }
}

pub type CommandResult<T> = std::result::Result<T, CommandError>;
