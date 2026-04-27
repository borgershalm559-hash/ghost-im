//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.
//!
//! This crate is built on top of `openmls` (RFC 9420 — MLS) and provides a
//! domain-friendly façade for Ghost's specific needs: 2-member groups for 1-on-1
//! conversations, sealed-sender envelopes that hide the sender from the recipient's
//! server, and asynchronous first-contact via published KeyPackages.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
