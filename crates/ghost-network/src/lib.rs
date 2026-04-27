//! Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery.
//!
//! Built on top of rust-libp2p 0.56. The network carries opaque bytes (CBOR envelopes
//! produced by ghost-protocol's `wrap_message`); it does not interpret payload content.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-network");
    }
}
