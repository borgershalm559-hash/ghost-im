//! Top-level error type for ghost-network.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("libp2p transport error: {0}")]
    Transport(String),

    #[error("dial failed for peer {peer}: {detail}")]
    DialFailed { peer: String, detail: String },

    #[error("peer authentication failed: expected {expected}, got {got}")]
    PeerAuthMismatch { expected: String, got: String },

    #[error("DHT query failed: {0}")]
    DhtQuery(String),

    #[error("record signature invalid for {0}")]
    InvalidSignature(String),

    #[error("record expired at {0}")]
    RecordExpired(u64),

    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("identity-key conversion: {0}")]
    KeyConversion(String),

    #[error("network task channel closed")]
    ChannelClosed,

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;
