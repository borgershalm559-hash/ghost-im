//! Common error category for ghost-core. Crate-specific errors live in their own crates.

use thiserror::Error;

/// Top-level error type that other Ghost crates can wrap.
/// Currently only used to expose GhostIdParseError; will grow.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid ghost id: {0}")]
    InvalidId(#[from] crate::id::GhostIdParseError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::GhostId;

    #[test]
    fn wraps_id_parse_error() {
        let err: CoreError = GhostId::from_bech32("garbage").unwrap_err().into();
        let msg = err.to_string();
        assert!(msg.starts_with("invalid ghost id:"));
    }
}
