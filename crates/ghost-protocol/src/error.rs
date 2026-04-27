//! Top-level error type for ghost-protocol. Wraps openmls errors plus our own
//! envelope/sealed-sender/uuid-parse failures.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("envelope wrong recipient: expected {expected}, got {got}")]
    WrongRecipient { expected: String, got: String },
    #[error("envelope unsupported version {0}")]
    UnsupportedVersion(u8),
    #[error("envelope unknown msg_type {0}")]
    UnknownMsgType(u8),

    #[error("sealed sender encrypt failed")]
    SealedSenderEncrypt,
    #[error("sealed sender decrypt failed (wrong key, tampered, or wrong recipient)")]
    SealedSenderDecrypt,

    #[error("invalid sender signature")]
    BadSenderSignature,
    #[error("duplicate msg_uuid (replay attack)")]
    Replay,

    #[error("MLS error: {0}")]
    Mls(String),

    #[error("ghost-identity error: {0}")]
    Identity(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ProtoError>;
