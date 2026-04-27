//! GhostInvite — bech32-encoded out-of-band first-contact bundle.

use crate::{ClientError, Result};
use bech32::{Bech32, Hrp};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use serde::{Deserialize, Serialize};

const HRP_INVITE: &str = "ghostinvite";

mod serde_sig {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        serde_bytes::Bytes::new(v).serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let bytes: serde_bytes::ByteBuf = Deserialize::deserialize(d)?;
        let bs = bytes.into_vec();
        if bs.len() != 64 {
            return Err(serde::de::Error::custom(format!(
                "expected 64, got {}",
                bs.len()
            )));
        }
        let mut a = [0u8; 64];
        a.copy_from_slice(&bs);
        Ok(a)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GhostInvite {
    pub ghost_id: GhostId,
    pub addresses: Vec<String>,
    pub invite_token: [u8; 16],
    pub expires_at: u64,
    #[serde(with = "serde_sig")]
    pub signature: [u8; 64],
}

impl GhostInvite {
    pub fn new(
        ik: &IdentityKey,
        addresses: Vec<String>,
        invite_token: [u8; 16],
        now: u64,
        ttl_seconds: u64,
    ) -> Self {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, &addresses, &invite_token, expires_at);
        let sig = ik.sign(&to_sign);
        Self {
            ghost_id,
            addresses,
            invite_token,
            expires_at,
            signature: sig.to_bytes(),
        }
    }

    pub fn signing_bytes(
        ghost_id: &GhostId,
        addresses: &[String],
        invite_token: &[u8; 16],
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        let n = addresses.len() as u32;
        hasher.update(&n.to_be_bytes());
        for a in addresses {
            let l = a.len() as u32;
            hasher.update(&l.to_be_bytes());
            hasher.update(a.as_bytes());
        }
        hasher.update(invite_token);
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(ClientError::InviteExpired(self.expires_at));
        }
        let pub_key = VerifyingKey::from_bytes(self.ghost_id.as_bytes())
            .map_err(|e| ClientError::Invalid(format!("ghost_id: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify = Self::signing_bytes(
            &self.ghost_id,
            &self.addresses,
            &self.invite_token,
            self.expires_at,
        );
        pub_key
            .verify(&to_verify, &sig)
            .map_err(|_| ClientError::InviteSignatureInvalid)
    }

    pub fn to_bech32(&self) -> Result<String> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ClientError::CborEncode(e.to_string()))?;
        let hrp = Hrp::parse(HRP_INVITE).expect("static HRP is valid");
        bech32::encode::<Bech32>(hrp, &buf)
            .map_err(|e| ClientError::Invalid(format!("bech32 encode: {e}")))
    }

    pub fn from_bech32(s: &str) -> Result<Self> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| ClientError::InviteParse(format!("bech32: {e}")))?;
        if hrp.as_str() != HRP_INVITE {
            return Err(ClientError::InviteParse(format!(
                "wrong hrp: expected {HRP_INVITE}, got {}",
                hrp.as_str()
            )));
        }
        ciborium::from_reader(&data[..]).map_err(|e| ClientError::CborDecode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_round_trip_via_bech32() {
        let ik = IdentityKey::generate();
        let invite = GhostInvite::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/0/quic-v1".into()],
            [7u8; 16],
            1700000000,
            3600 * 24 * 7,
        );
        let s = invite.to_bech32().unwrap();
        assert!(s.starts_with("ghostinvite1"));
        let restored = GhostInvite::from_bech32(&s).unwrap();
        assert_eq!(restored, invite);
        restored.verify(1700000100).unwrap();
    }

    #[test]
    fn verify_fails_when_expired() {
        let ik = IdentityKey::generate();
        let invite = GhostInvite::new(&ik, vec![], [0u8; 16], 1700000000, 60);
        let err = invite.verify(1700001000).unwrap_err();
        assert!(matches!(err, ClientError::InviteExpired(_)));
    }

    #[test]
    fn verify_fails_on_tampered_addresses() {
        let ik = IdentityKey::generate();
        let mut invite = GhostInvite::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/0/quic-v1".into()],
            [0u8; 16],
            0,
            1000,
        );
        invite.addresses.push("/ip4/9.9.9.9/udp/0/quic-v1".into());
        let err = invite.verify(0).unwrap_err();
        assert!(matches!(err, ClientError::InviteSignatureInvalid));
    }

    #[test]
    fn from_bech32_rejects_wrong_hrp() {
        let hrp = Hrp::parse("ghost").unwrap();
        let bad = bech32::encode::<Bech32>(hrp, &[1u8; 4]).unwrap();
        let err = GhostInvite::from_bech32(&bad).unwrap_err();
        assert!(matches!(err, ClientError::InviteParse(_)));
    }
}
