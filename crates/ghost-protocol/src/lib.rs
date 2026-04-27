//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
