//! Sealed-sender layer.
//!
//! The recipient publishes an X25519 delivery public key (derived deterministically from their IK).
//! Senders ECDH against this delivery pubkey to wrap the SealedBlob, hiding the sender ID from
//! the network and from the recipient's own server (relevant once we have federation in MVP-3+).

use ghost_identity::IdentityKey;
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

const HKDF_SALT: &[u8] = b"ghost.delivery.v1.salt";
const HKDF_INFO: &[u8] = b"ghost.delivery.v1.x25519";

/// Derive the X25519 delivery secret from an IdentityKey.
/// Holders of the IdentityKey can derive both halves; outside parties cannot derive the secret
/// (since it depends on IK's private seed). For others, the public delivery key is published
/// separately (e.g., via the `/v1/delivery-key/<ghostid>` server endpoint in MVP-3+).
pub fn delivery_secret(ik: &IdentityKey) -> X25519Secret {
    let seed = ik.secret_bytes();
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), &seed);
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("32-byte expand always succeeds");
    X25519Secret::from(okm)
}

pub fn delivery_public(ik: &IdentityKey) -> X25519Public {
    X25519Public::from(&delivery_secret(ik))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_keys_are_deterministic_per_identity() {
        let ik = IdentityKey::generate();
        let s1 = delivery_secret(&ik);
        let s2 = delivery_secret(&ik);
        let p1 = X25519Public::from(&s1);
        let p2 = X25519Public::from(&s2);
        assert_eq!(p1.as_bytes(), p2.as_bytes());
    }

    #[test]
    fn distinct_identities_yield_distinct_delivery_keys() {
        let ik_a = IdentityKey::generate();
        let ik_b = IdentityKey::generate();
        let pa = delivery_public(&ik_a);
        let pb = delivery_public(&ik_b);
        assert_ne!(pa.as_bytes(), pb.as_bytes());
    }

    #[test]
    fn delivery_public_matches_secret_derivation() {
        let ik = IdentityKey::generate();
        let secret = delivery_secret(&ik);
        let pub_via_secret = X25519Public::from(&secret);
        let pub_direct = delivery_public(&ik);
        assert_eq!(pub_via_secret.as_bytes(), pub_direct.as_bytes());
    }
}
