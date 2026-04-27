//! GhostId — 32-byte Ed25519 public key serving as the user's globally unique identity.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GhostId([u8; 32]);

impl GhostId {
    pub const SIZE: usize = 32;

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Debug for GhostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Truncated for safety in logs — never print full key in Debug.
        write!(f, "GhostId({}…)", hex::encode(&self.0[..4]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let bytes = [42u8; 32];
        let id = GhostId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
        assert_eq!(id.into_bytes(), bytes);
    }

    #[test]
    fn debug_truncates_to_avoid_leaking_full_key() {
        let id = GhostId::from_bytes([0xAB; 32]);
        let debug_str = format!("{:?}", id);
        assert!(debug_str.starts_with("GhostId(abababab"));
        assert!(debug_str.ends_with("…)"));
        // Must NOT contain the full hex (64 chars).
        assert!(!debug_str.contains(&"ab".repeat(32)));
    }

    #[test]
    fn equality_and_hash() {
        use std::collections::HashSet;
        let a = GhostId::from_bytes([1u8; 32]);
        let b = GhostId::from_bytes([1u8; 32]);
        let c = GhostId::from_bytes([2u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
