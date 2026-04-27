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

    /// Expose the 32-byte secret seed. Must NEVER be used for anything other than
    /// deriving deterministic subkeys (e.g., delivery key) within trusted code in
    /// this workspace. The raw seed must never leave the process.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
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

/// Device Key — Ed25519 keypair signed by the parent IdentityKey.
/// In MVP-1 there is one DK per identity; the architecture supports adding more later.
#[derive(Serialize, Deserialize)]
pub struct DeviceKey {
    signing: SigningKey,
    /// Signature by the parent IdentityKey over `signing.verifying_key().to_bytes()`.
    parent_signature: Signature,
}

impl DeviceKey {
    /// Generate a fresh DeviceKey signed by the given IdentityKey.
    pub fn generate(parent: &IdentityKey) -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        let pubkey_bytes = signing.verifying_key().to_bytes();
        let parent_signature = parent.sign(&pubkey_bytes);
        Self {
            signing,
            parent_signature,
        }
    }

    pub fn public(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    pub fn parent_signature(&self) -> &Signature {
        &self.parent_signature
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }

    pub fn verify_parent(&self, parent_pub: &VerifyingKey) -> bool {
        let pubkey_bytes = self.signing.verifying_key().to_bytes();
        parent_pub
            .verify(&pubkey_bytes, &self.parent_signature)
            .is_ok()
    }
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pub_short = hex::encode(&self.signing.verifying_key().to_bytes()[..4]);
        write!(f, "DeviceKey(pub={}…)", pub_short)
    }
}

#[cfg(test)]
mod device_key_tests {
    use super::*;

    #[test]
    fn dk_signature_verifies_against_parent() {
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        assert!(dk.verify_parent(&ik.public()));
    }

    #[test]
    fn dk_signature_rejects_wrong_parent() {
        let ik_a = IdentityKey::generate();
        let ik_b = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik_a);
        assert!(!dk.verify_parent(&ik_b.public()));
    }

    #[test]
    fn dk_signs_messages_independently() {
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let sig = dk.sign(b"message-from-device");
        assert!(dk.public().verify(b"message-from-device", &sig).is_ok());
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
