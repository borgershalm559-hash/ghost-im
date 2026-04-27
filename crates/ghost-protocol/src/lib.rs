//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod envelope;
pub mod error;
pub mod key_package;
pub mod mls_credential;
pub mod mls_provider;
pub mod mls_session;
pub mod msg_uuid;
pub mod outer_envelope;
pub mod sealed_blob;
pub mod sealed_sender;

pub use envelope::{unwrap_message, wrap_message, UnwrappedMessage};
pub use error::{ProtoError, Result};
pub use key_package::{generate_key_package, GHOST_CIPHERSUITE};
pub use mls_credential::credential_with_key;
pub use mls_provider::{new_provider, GhostMlsProvider};
pub use mls_session::MlsSession;
pub use msg_uuid::MessageUuid;
pub use outer_envelope::{MsgType, OuterEnvelope, PROTOCOL_VERSION};
pub use sealed_blob::{PayloadType, SealedBlob};
pub use sealed_sender::{delivery_public, delivery_secret};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
