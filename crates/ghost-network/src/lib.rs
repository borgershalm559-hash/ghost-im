//! Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery.

pub mod address_record;
pub mod behaviour;
pub mod error;
pub mod identity;
pub mod presence_record;

pub use address_record::AddressRecord;
pub use behaviour::{GhostBehaviour, GhostMessageCodec, GHOST_PROTOCOL};
pub use error::{NetworkError, Result};
pub use identity::{ghost_id_from_peer_id, libp2p_keypair_from_ik, peer_id_from_ghost_id};
pub use presence_record::PresenceRecord;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-network");
    }
}
