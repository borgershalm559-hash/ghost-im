//! Ghost embedded server: dispatches typed requests over the network.

pub mod error;
pub mod messages;

pub use error::{Result, ServerError};
pub use messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-server");
    }
}
