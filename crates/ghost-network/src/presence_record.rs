//! PresenceRecord: signed online-status advertisement.

use crate::{NetworkError, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
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
                v.try_into()
                    .map_err(|_| E::invalid_length(v.len(), &"64 bytes"))
            }
        }
        d.deserialize_bytes(Visitor)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresenceRecord {
    pub ghost_id: GhostId,
    pub online: bool,
    pub last_seen: u64,
    pub expires_at: u64,
    #[serde(with = "serde_sig")]
    pub signature: [u8; 64],
}

impl PresenceRecord {
    pub fn new(ik: &IdentityKey, online: bool, now: u64, ttl_seconds: u64) -> Result<Self> {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, online, now, expires_at);
        let sig = ik.sign(&to_sign);
        Ok(Self {
            ghost_id,
            online,
            last_seen: now,
            expires_at,
            signature: sig.to_bytes(),
        })
    }

    pub fn signing_bytes(
        ghost_id: &GhostId,
        online: bool,
        last_seen: u64,
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        hasher.update(&[online as u8]);
        hasher.update(&last_seen.to_be_bytes());
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(NetworkError::RecordExpired(self.expires_at));
        }
        let pub_key = VerifyingKey::from_bytes(self.ghost_id.as_bytes())
            .map_err(|e| NetworkError::Invalid(format!("ghost_id: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify =
            Self::signing_bytes(&self.ghost_id, self.online, self.last_seen, self.expires_at);
        pub_key
            .verify(&to_verify, &sig)
            .map_err(|_| NetworkError::InvalidSignature(format!("{}", self.ghost_id)))
    }

    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| NetworkError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| NetworkError::CborDecode(e.to_string()))
    }

    pub fn dht_key(ghost_id: &GhostId) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        hasher.update(b"presence/v1");
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_signing_and_verify() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, true, 1000, 90).unwrap();
        r.verify(1050).unwrap();
        assert!(r.online);
        assert_eq!(r.last_seen, 1000);
        assert_eq!(r.expires_at, 1090);
    }

    #[test]
    fn presence_verify_fails_after_expiry() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, true, 0, 60).unwrap();
        let err = r.verify(100).unwrap_err();
        assert!(matches!(err, NetworkError::RecordExpired(_)));
    }

    #[test]
    fn presence_dht_key_distinct_from_address_dht_key() {
        let id = GhostId::from_bytes([3u8; 32]);
        let presence_key = PresenceRecord::dht_key(&id);
        let address_key = crate::address_record::AddressRecord::dht_key(&id);
        assert_ne!(presence_key, address_key);
    }

    #[test]
    fn cbor_roundtrip() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, false, 555, 90).unwrap();
        let bytes = r.to_cbor().unwrap();
        let decoded = PresenceRecord::from_cbor(&bytes).unwrap();
        assert_eq!(decoded, r);
    }
}
