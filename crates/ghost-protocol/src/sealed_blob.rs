//! SealedBlob — inner ciphertext payload, encrypted to recipient's delivery key.
//!
//! Layout (CBOR-serialized then encrypted by the sealed-sender layer):
//! ```text
//! SealedBlob {
//!   sender_id:        GhostId         // the real sender (hidden from server)
//!   payload_type:     u8              // 0=text app, 1=mls handshake, 2=mls commit, ...
//!   payload:          Bytes           // typically MLSMessage from openmls
//!   msg_uuid:         MessageUuid     // for dedup
//!   sender_signature: [u8; 64]        // Ed25519(DK, hash(sender_id || payload_type || payload || msg_uuid))
//! }
//! ```

use crate::msg_uuid::MessageUuid;
use crate::{ProtoError, Result};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

/// Custom serde for `[u8; 64]`, which serde does not support out of the box.
mod serde_sig {
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = [u8; 64];
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "64 bytes")
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                v.try_into().map_err(|_| {
                    E::invalid_length(v.len(), &"64 bytes")
                })
            }
        }
        d.deserialize_bytes(Visitor)
    }
}

/// Payload-type tag inside a SealedBlob.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadType {
    AppText = 0,
    MlsHandshake = 1,
    MlsCommit = 2,
    Ack = 3,
}

impl TryFrom<u8> for PayloadType {
    type Error = ProtoError;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::AppText),
            1 => Ok(Self::MlsHandshake),
            2 => Ok(Self::MlsCommit),
            3 => Ok(Self::Ack),
            other => Err(ProtoError::Invalid(format!("unknown payload type {other}"))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedBlob {
    pub sender_id: GhostId,
    pub payload_type: PayloadType,
    pub payload: Vec<u8>,
    pub msg_uuid: MessageUuid,
    #[serde(with = "serde_sig")]
    pub sender_signature: [u8; 64],
}

impl SealedBlob {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).map_err(|e| ProtoError::CborEncode(e.to_string()))?;
        Ok(out)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ProtoError::CborDecode(e.to_string()))
    }

    /// Compute the bytes that the sender must sign (with their DK):
    /// blake3(sender_id || payload_type || payload || msg_uuid)
    pub fn signing_bytes(
        sender: &GhostId,
        payload_type: PayloadType,
        payload: &[u8],
        msg_uuid: &MessageUuid,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(sender.as_bytes());
        hasher.update(&[payload_type as u8]);
        hasher.update(payload);
        hasher.update(msg_uuid.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn fake_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn cbor_roundtrip_preserves_all_fields() {
        let sender = GhostId::from_bytes([7u8; 32]);
        let uuid = MessageUuid::new();
        let payload = b"hello world".to_vec();
        let signing_bytes = SealedBlob::signing_bytes(&sender, PayloadType::AppText, &payload, &uuid);
        let dk = fake_signing_key();
        let sig = dk.sign(&signing_bytes).to_bytes();

        let original = SealedBlob {
            sender_id: sender,
            payload_type: PayloadType::AppText,
            payload: payload.clone(),
            msg_uuid: uuid,
            sender_signature: sig,
        };

        let bytes = original.to_cbor().unwrap();
        let decoded = SealedBlob::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.sender_id, original.sender_id);
        assert_eq!(decoded.payload_type, original.payload_type);
        assert_eq!(decoded.payload, original.payload);
        assert_eq!(decoded.msg_uuid, original.msg_uuid);
        assert_eq!(decoded.sender_signature, original.sender_signature);
    }

    #[test]
    fn payload_type_try_from() {
        assert_eq!(PayloadType::try_from(0u8).unwrap(), PayloadType::AppText);
        assert_eq!(PayloadType::try_from(1u8).unwrap(), PayloadType::MlsHandshake);
        assert!(matches!(
            PayloadType::try_from(99u8),
            Err(ProtoError::Invalid(_))
        ));
    }

    #[test]
    fn signing_bytes_deterministic() {
        let s = GhostId::from_bytes([1u8; 32]);
        let u = MessageUuid::from_bytes([2u8; 16]);
        let p = b"data";
        let a = SealedBlob::signing_bytes(&s, PayloadType::AppText, p, &u);
        let b = SealedBlob::signing_bytes(&s, PayloadType::AppText, p, &u);
        assert_eq!(a, b);
    }

    #[test]
    fn signing_bytes_differ_when_input_differs() {
        let s = GhostId::from_bytes([1u8; 32]);
        let u = MessageUuid::from_bytes([2u8; 16]);
        let a = SealedBlob::signing_bytes(&s, PayloadType::AppText, b"data1", &u);
        let b = SealedBlob::signing_bytes(&s, PayloadType::AppText, b"data2", &u);
        assert_ne!(a, b);
    }

    #[test]
    fn from_cbor_rejects_garbage() {
        let err = SealedBlob::from_cbor(b"not valid cbor").unwrap_err();
        assert!(matches!(err, ProtoError::CborDecode(_)));
    }
}
