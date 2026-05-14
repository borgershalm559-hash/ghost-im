//! Top-level error type for ghost-client.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("identity: {0}")]
    Identity(#[from] ghost_identity::IdentityError),
    #[error("storage: {0}")]
    Storage(#[from] ghost_storage::StorageError),
    #[error("network: {0}")]
    Network(#[from] ghost_network::NetworkError),
    #[error("server: {0}")]
    Server(#[from] ghost_server::ServerError),
    #[error("protocol: {0}")]
    Protocol(#[from] ghost_protocol::ProtoError),

    #[error("invite parse: {0}")]
    InviteParse(String),
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
    #[error("invite expired at {0}")]
    InviteExpired(u64),

    #[error("contact not found: {0}")]
    ContactNotFound(String),
    #[error("MLS group not found for contact {0}")]
    MlsGroupNotFound(String),

    #[error("no key packages available from peer")]
    NoKeyPackagesFromPeer,

    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    /// Friendly variant for libp2p dial failures — kept distinct from
    /// generic `Network(...)` so the UI can show a non-technical message.
    #[error("Не удалось соединиться: {0}")]
    PeerUnreachable(String),

    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;
