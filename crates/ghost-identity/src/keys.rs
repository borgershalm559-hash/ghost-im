//! Identity Key (IK) and Device Key (DK).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH};
use ghost_core::GhostId;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Master Identity Key. Lives in identity.encrypted. Never sent over the wire.
#[derive(Serialize, Deserialize)]
pub struct IdentityKey {
    signing: SigningKey,
}

impl IdentityKey {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    /// Reconstruct from raw secret seed (used during deserialization or restore).
    pub fn from_secret_bytes(secret: [u8; SECRET_KEY_LENGTH]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&secret),
        }
    }

    pub fn public(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    pub fn ghost_id(&self) -> GhostId {
        GhostId::from_bytes(self.signing.verifying_key().to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }

    pub fn verify(&self, message: &[u8], sig: &Signature) -> bool {
        self.public().verify(message, sig).is_ok()
    }
}

impl Drop for IdentityKey {
    fn drop(&mut self) {
        // SigningKey already zeroizes via its own Drop; explicit pin for clarity.
        let mut bytes = self.signing.to_bytes();
        bytes.zeroize();
    }
}

impl std::fmt::Debug for IdentityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IdentityKey(ghost_id={:?})", self.ghost_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_distinct_keys() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        assert_ne!(a.ghost_id(), b.ghost_id());
    }

    #[test]
    fn ghost_id_equals_public_bytes() {
        let key = IdentityKey::generate();
        let id = key.ghost_id();
        assert_eq!(id.as_bytes(), &key.public().to_bytes());
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let key = IdentityKey::generate();
        let message = b"hello ghost";
        let sig = key.sign(message);
        assert!(key.verify(message, &sig));
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let key = IdentityKey::generate();
        let sig = key.sign(b"original");
        assert!(!key.verify(b"tampered", &sig));
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let key = IdentityKey::generate();
        let dbg = format!("{:?}", key);
        // Should reference ghost_id (truncated) but not raw secret.
        assert!(dbg.starts_with("IdentityKey(ghost_id="));
        // Sanity: ghost_id Debug already truncates to first 4 bytes.
        assert!(dbg.contains("…"));
    }
}
