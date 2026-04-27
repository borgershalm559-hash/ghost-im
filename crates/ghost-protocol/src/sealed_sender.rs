//! Sealed-sender layer.
//!
//! The recipient publishes an X25519 delivery public key (derived deterministically from their IK).
//! Senders ECDH against this delivery pubkey to wrap the SealedBlob, hiding the sender ID from
//! the network and from the recipient's own server (relevant once we have federation in MVP-3+).

use crate::{ProtoError, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use ghost_identity::IdentityKey;
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519Public, StaticSecret as X25519Secret};

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

const AEAD_AAD: &[u8] = b"ghost.sealed_sender.v1.aad";
const NONCE_LEN: usize = 24;
const EPHEMERAL_PUB_LEN: usize = 32;

/// Encrypt `plaintext` (typically a CBOR-encoded SealedBlob) so that only the holder of
/// the IK that produced `recipient_delivery_pub` can decrypt.
///
/// Output layout: `eph_pub (32) || nonce (24) || ciphertext+tag`
pub fn seal_to(recipient_delivery_pub: &X25519Public, plaintext: &[u8]) -> Result<Vec<u8>> {
    let eph_secret = EphemeralSecret::random_from_rng(&mut rand::thread_rng());
    let eph_pub = X25519Public::from(&eph_secret);
    let shared = eph_secret.diffie_hellman(recipient_delivery_pub);

    let mut aead_key = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(b"ghost.sealed_sender.v1.kdf"), shared.as_bytes());
    hk.expand(b"aead-key", &mut aead_key)
        .expect("expand 32 always succeeds");

    let cipher = XChaCha20Poly1305::new((&aead_key).into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: AEAD_AAD,
            },
        )
        .map_err(|_| ProtoError::SealedSenderEncrypt)?;

    let mut out = Vec::with_capacity(EPHEMERAL_PUB_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(eph_pub.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a sealed-sender blob using our delivery secret.
pub fn unseal(recipient_delivery_secret: &X25519Secret, sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < EPHEMERAL_PUB_LEN + NONCE_LEN {
        return Err(ProtoError::SealedSenderDecrypt);
    }
    let mut eph_bytes = [0u8; EPHEMERAL_PUB_LEN];
    eph_bytes.copy_from_slice(&sealed[..EPHEMERAL_PUB_LEN]);
    let eph_pub = X25519Public::from(eph_bytes);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&sealed[EPHEMERAL_PUB_LEN..EPHEMERAL_PUB_LEN + NONCE_LEN]);

    let ciphertext = &sealed[EPHEMERAL_PUB_LEN + NONCE_LEN..];

    let shared = recipient_delivery_secret.diffie_hellman(&eph_pub);
    let mut aead_key = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(b"ghost.sealed_sender.v1.kdf"), shared.as_bytes());
    hk.expand(b"aead-key", &mut aead_key)
        .expect("expand 32 always succeeds");

    let cipher = XChaCha20Poly1305::new((&aead_key).into());
    let nonce = XNonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: AEAD_AAD,
            },
        )
        .map_err(|_| ProtoError::SealedSenderDecrypt)
}

#[cfg(test)]
mod sealing_tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        let recipient = IdentityKey::generate();
        let recipient_pub = delivery_public(&recipient);
        let recipient_secret = delivery_secret(&recipient);

        let plaintext = b"the inner sealed blob bytes";
        let sealed = seal_to(&recipient_pub, plaintext).unwrap();
        assert!(sealed.len() >= 32 + 24 + 16);
        let recovered = unseal(&recipient_secret, &sealed).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn unseal_fails_with_wrong_recipient() {
        let intended = IdentityKey::generate();
        let stranger = IdentityKey::generate();
        let sealed = seal_to(&delivery_public(&intended), b"data").unwrap();
        let err = unseal(&delivery_secret(&stranger), &sealed).unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn unseal_fails_on_truncated_input() {
        let recipient = IdentityKey::generate();
        let err = unseal(&delivery_secret(&recipient), b"too short").unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn unseal_fails_on_tampered_ciphertext() {
        let recipient = IdentityKey::generate();
        let mut sealed = seal_to(&delivery_public(&recipient), b"data").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0xFF;
        let err = unseal(&delivery_secret(&recipient), &sealed).unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn each_seal_uses_fresh_ephemeral() {
        let recipient = IdentityKey::generate();
        let pub_ = delivery_public(&recipient);
        let s1 = seal_to(&pub_, b"data").unwrap();
        let s2 = seal_to(&pub_, b"data").unwrap();
        assert_ne!(&s1[..32], &s2[..32]);
    }
}
