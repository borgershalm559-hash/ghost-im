//! AddressRecord: signed advertisement of (GhostId, endpoints, expiry).
//!
//! Published to Kademlia DHT under key BLAKE3(ghost_id). DHT nodes verify the signature
//! against the GhostId before accepting the record.

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
pub struct AddressRecord {
    pub ghost_id: GhostId,
    pub endpoints: Vec<String>,
    pub expires_at: u64,
    #[serde(with = "serde_sig")]
    pub signature: [u8; 64],
}

impl AddressRecord {
    pub fn new(
        ik: &IdentityKey,
        endpoints: Vec<String>,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<Self> {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, &endpoints, expires_at);
        let sig = ik.sign(&to_sign);
        Ok(Self {
            ghost_id,
            endpoints,
            expires_at,
            signature: sig.to_bytes(),
        })
    }

    pub fn signing_bytes(
        ghost_id: &GhostId,
        endpoints: &[String],
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        let n = endpoints.len() as u32;
        hasher.update(&n.to_be_bytes());
        for ep in endpoints {
            let len = ep.len() as u32;
            hasher.update(&len.to_be_bytes());
            hasher.update(ep.as_bytes());
        }
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(NetworkError::RecordExpired(self.expires_at));
        }
        let pub_key = VerifyingKey::from_bytes(self.ghost_id.as_bytes())
            .map_err(|e| NetworkError::Invalid(format!("ghost_id ed25519: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify = Self::signing_bytes(&self.ghost_id, &self.endpoints, self.expires_at);
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
        *blake3::hash(ghost_id.as_bytes()).as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_record_verifies_against_own_signature() {
        let ik = IdentityKey::generate();
        let r = AddressRecord::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_string()],
            1700000000,
            300,
        )
        .unwrap();
        r.verify(1700000100).unwrap();
    }

    #[test]
    fn verify_fails_when_expired() {
        let ik = IdentityKey::generate();
        let r = AddressRecord::new(&ik, vec![], 1700000000, 60).unwrap();
        let err = r.verify(1700000100).unwrap_err();
        assert!(matches!(err, NetworkError::RecordExpired(_)));
    }

    #[test]
    fn verify_fails_when_signature_tampered() {
        let ik = IdentityKey::generate();
        let mut r = AddressRecord::new(&ik, vec!["/ip4/1.2.3.4/udp/1/quic-v1".into()], 0, 1000)
            .unwrap();
        r.signature[0] ^= 0xFF;
        let err = r.verify(0).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidSignature(_)));
    }

    #[test]
    fn verify_fails_when_endpoints_tampered_after_signing() {
        let ik = IdentityKey::generate();
        let mut r = AddressRecord::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/1/quic-v1".to_string()],
            0,
            1000,
        )
        .unwrap();
        r.endpoints.push("/ip4/9.9.9.9/udp/2/quic-v1".to_string());
        let err = r.verify(0).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidSignature(_)));
    }

    #[test]
    fn cbor_roundtrip() {
        let ik = IdentityKey::generate();
        let original = AddressRecord::new(
            &ik,
            vec!["/ip4/1.2.3.4/udp/1/quic-v1".into(), "/ip6/::1/udp/2/quic-v1".into()],
            123,
            456,
        )
        .unwrap();
        let bytes = original.to_cbor().unwrap();
        let decoded = AddressRecord::from_cbor(&bytes).unwrap();
        assert_eq!(decoded, original);
        decoded.verify(200).unwrap();
    }

    #[test]
    fn dht_key_is_blake3_of_ghost_id() {
        let id = GhostId::from_bytes([7u8; 32]);
        let key = AddressRecord::dht_key(&id);
        assert_eq!(key, *blake3::hash(id.as_bytes()).as_bytes());
    }
}
