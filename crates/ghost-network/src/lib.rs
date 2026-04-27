//! Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery.

pub mod error;

pub use error::{NetworkError, Result};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-network");
    }
}
