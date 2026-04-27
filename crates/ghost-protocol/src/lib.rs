//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;

pub use error::{ProtoError, Result};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
