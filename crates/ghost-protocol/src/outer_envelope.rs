//! OuterEnvelope — the visible wire envelope. Recipient and timestamp are clear-text;
//! the entire SealedBlob (sender + payload) is encrypted to the recipient's delivery key.

use crate::{ProtoError, Result};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgType {
    AppMessage = 0,
    MlsHandshake = 1,
    Ack = 2,
}

impl TryFrom<u8> for MsgType {
    type Error = ProtoError;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::AppMessage),
            1 => Ok(Self::MlsHandshake),
            2 => Ok(Self::Ack),
            other => Err(ProtoError::UnknownMsgType(other)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OuterEnvelope {
    pub version: u8,
    pub msg_type: MsgType,
    pub recipient: GhostId,
    /// UTC seconds, rounded.
    pub timestamp: u64,
    /// Encrypted SealedBlob. Decryptable only by `recipient`'s delivery key.
    pub sealed_blob: Vec<u8>,
}

impl OuterEnvelope {
    pub fn new(
        msg_type: MsgType,
        recipient: GhostId,
        timestamp: u64,
        sealed_blob: Vec<u8>,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            msg_type,
            recipient,
            timestamp,
            sealed_blob,
        }
    }

    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).map_err(|e| ProtoError::CborEncode(e.to_string()))?;
        Ok(out)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        let env: Self =
            ciborium::from_reader(bytes).map_err(|e| ProtoError::CborDecode(e.to_string()))?;
        if env.version != PROTOCOL_VERSION {
            return Err(ProtoError::UnsupportedVersion(env.version));
        }
        Ok(env)
    }

    pub fn check_recipient(&self, expected_recipient: &GhostId) -> Result<()> {
        if &self.recipient != expected_recipient {
            return Err(ProtoError::WrongRecipient {
                expected: format!("{}", expected_recipient),
                got: format!("{}", self.recipient),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_roundtrip() {
        let recipient = GhostId::from_bytes([5u8; 32]);
        let original =
            OuterEnvelope::new(MsgType::AppMessage, recipient, 1700000000, vec![9, 9, 9]);
        let bytes = original.to_cbor().unwrap();
        let decoded = OuterEnvelope::from_cbor(&bytes).unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.msg_type, MsgType::AppMessage);
        assert_eq!(decoded.recipient, recipient);
        assert_eq!(decoded.timestamp, 1700000000);
        assert_eq!(decoded.sealed_blob, vec![9, 9, 9]);
    }

    #[test]
    fn from_cbor_rejects_unsupported_version() {
        let recipient = GhostId::from_bytes([5u8; 32]);
        let mut bad = OuterEnvelope::new(MsgType::AppMessage, recipient, 1, vec![]);
        bad.version = 99;
        let bytes = bad.to_cbor().unwrap();
        let err = OuterEnvelope::from_cbor(&bytes).unwrap_err();
        assert!(matches!(err, ProtoError::UnsupportedVersion(99)));
    }

    #[test]
    fn check_recipient_accepts_match() {
        let r = GhostId::from_bytes([7u8; 32]);
        let env = OuterEnvelope::new(MsgType::AppMessage, r, 1, vec![]);
        env.check_recipient(&r).unwrap();
    }

    #[test]
    fn check_recipient_rejects_mismatch() {
        let alice = GhostId::from_bytes([1u8; 32]);
        let bob = GhostId::from_bytes([2u8; 32]);
        let env = OuterEnvelope::new(MsgType::AppMessage, alice, 1, vec![]);
        let err = env.check_recipient(&bob).unwrap_err();
        assert!(matches!(err, ProtoError::WrongRecipient { .. }));
    }

    #[test]
    fn msg_type_try_from() {
        assert_eq!(MsgType::try_from(0u8).unwrap(), MsgType::AppMessage);
        assert_eq!(MsgType::try_from(1u8).unwrap(), MsgType::MlsHandshake);
        assert!(matches!(
            MsgType::try_from(50u8),
            Err(ProtoError::UnknownMsgType(50))
        ));
    }
}
