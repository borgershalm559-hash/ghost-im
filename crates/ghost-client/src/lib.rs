//! Ghost client orchestration.

pub mod client;
pub mod error;
pub mod invite;

pub use client::{Client, ClientConfig};
pub use error::{ClientError, Result};
pub use invite::GhostInvite;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-client");
    }
}
