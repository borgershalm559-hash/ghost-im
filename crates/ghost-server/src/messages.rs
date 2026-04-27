//! Typed request/response enums for the embedded server.
//!
//! Carried as opaque bytes by ghost-network's request-response protocol.
//! Both sides CBOR-encode/decode at this layer.

use crate::{Result, ServerError};
use serde::{Deserialize, Serialize};

/// Current protocol version. Bumped on breaking wire-format changes.
pub const PROTOCOL_VERSION: &str = "ghost/1";
/// Minimum compatible version. Peers below this will be rejected.
pub const MIN_COMPAT_VERSION: &str = "ghost/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostRequest {
    Version,
    GetDeliveryKey,
    GetKeyPackage,
    GetPresence,
    InboxMessage { envelope: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostResponse {
    Version {
        protocol: String,
        min_compat: String,
    },
    DeliveryKey {
        x25519_pub: [u8; 32],
    },
    KeyPackage {
        bytes: Vec<u8>,
    },
    Presence {
        online: bool,
        last_seen: u64,
    },
    InboxAck,
    Error {
        reason: String,
    },
}

impl GhostRequest {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ServerError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ServerError::CborDecode(e.to_string()))
    }
}

impl GhostResponse {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ServerError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ServerError::CborDecode(e.to_string()))
    }

    /// Convert a successful response into a Result. `Error` variant becomes `Err`.
    pub fn into_ok(self) -> Result<Self> {
        match self {
            Self::Error { reason } => Err(ServerError::Remote(reason)),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_cbor_roundtrip_for_each_variant() {
        let cases = vec![
            GhostRequest::Version,
            GhostRequest::GetDeliveryKey,
            GhostRequest::GetKeyPackage,
            GhostRequest::GetPresence,
            GhostRequest::InboxMessage { envelope: vec![1, 2, 3, 4] },
        ];
        for req in cases {
            let bytes = req.to_cbor().unwrap();
            let _ = GhostRequest::from_cbor(&bytes).unwrap();
        }
    }

    #[test]
    fn response_cbor_roundtrip_for_each_variant() {
        let cases = vec![
            GhostResponse::Version {
                protocol: PROTOCOL_VERSION.to_string(),
                min_compat: MIN_COMPAT_VERSION.to_string(),
            },
            GhostResponse::DeliveryKey { x25519_pub: [9u8; 32] },
            GhostResponse::KeyPackage { bytes: vec![5, 6, 7] },
            GhostResponse::Presence { online: true, last_seen: 1234 },
            GhostResponse::InboxAck,
            GhostResponse::Error { reason: "test".to_string() },
        ];
        for resp in cases {
            let bytes = resp.to_cbor().unwrap();
            let _ = GhostResponse::from_cbor(&bytes).unwrap();
        }
    }

    #[test]
    fn into_ok_passes_through_success() {
        let r = GhostResponse::InboxAck;
        let r = r.into_ok().unwrap();
        assert!(matches!(r, GhostResponse::InboxAck));
    }

    #[test]
    fn into_ok_fails_on_error_variant() {
        let r = GhostResponse::Error { reason: "boom".to_string() };
        let err = r.into_ok().unwrap_err();
        assert!(matches!(err, ServerError::Remote(_)));
    }
}
