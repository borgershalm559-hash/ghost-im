//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;
pub mod outer_envelope;
pub mod sealed_blob;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;
pub use outer_envelope::{MsgType, OuterEnvelope, PROTOCOL_VERSION};
pub use sealed_blob::{PayloadType, SealedBlob};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
