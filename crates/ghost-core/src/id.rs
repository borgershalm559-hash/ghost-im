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

use bech32::{Bech32, Hrp};

/// Human-readable prefix for Ghost IDs in bech32 encoding.
pub const HRP_GHOST: &str = "ghost";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GhostIdParseError {
    #[error("invalid bech32 encoding: {0}")]
    InvalidBech32(String),
    #[error("expected hrp '{}', got '{0}'", HRP_GHOST)]
    WrongHrp(String),
    #[error("expected {expected} bytes after decoding, got {actual}")]
    WrongLength { expected: usize, actual: usize },
}

impl GhostId {
    /// Encode this GhostId as a bech32 string with `ghost1…` prefix.
    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(HRP_GHOST).expect("static HRP is valid");
        bech32::encode::<Bech32>(hrp, &self.0).expect("32 bytes always encodable")
    }

    /// Parse a bech32-encoded GhostId. Errors on wrong hrp or wrong length.
    pub fn from_bech32(s: &str) -> Result<Self, GhostIdParseError> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| GhostIdParseError::InvalidBech32(e.to_string()))?;
        if hrp.as_str() != HRP_GHOST {
            return Err(GhostIdParseError::WrongHrp(hrp.as_str().to_string()));
        }
        if data.len() != Self::SIZE {
            return Err(GhostIdParseError::WrongLength {
                expected: Self::SIZE,
                actual: data.len(),
            });
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&data);
        Ok(Self(bytes))
    }
}

impl std::fmt::Display for GhostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_bech32())
    }
}

impl std::str::FromStr for GhostId {
    type Err = GhostIdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bech32(s)
    }
}

#[cfg(test)]
mod bech32_tests {
    use super::*;

    #[test]
    fn encode_starts_with_ghost1() {
        let id = GhostId::from_bytes([0u8; 32]);
        let encoded = id.to_bech32();
        assert!(encoded.starts_with("ghost1"), "got: {}", encoded);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = GhostId::from_bytes([0xDEu8; 32]);
        let encoded = original.to_bech32();
        let decoded = GhostId::from_bech32(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn decode_rejects_wrong_hrp() {
        // Encode with a different HRP and try to parse as ghost.
        let hrp = Hrp::parse("npub").unwrap();
        let bad = bech32::encode::<Bech32>(hrp, &[0u8; 32]).unwrap();
        let err = GhostId::from_bech32(&bad).unwrap_err();
        assert!(matches!(err, GhostIdParseError::WrongHrp(_)));
    }

    #[test]
    fn decode_rejects_wrong_length() {
        // Encode 16 bytes with ghost hrp.
        let hrp = Hrp::parse(HRP_GHOST).unwrap();
        let short = bech32::encode::<Bech32>(hrp, &[0u8; 16]).unwrap();
        let err = GhostId::from_bech32(&short).unwrap_err();
        assert!(matches!(
            err,
            GhostIdParseError::WrongLength { expected: 32, actual: 16 }
        ));
    }

    #[test]
    fn decode_rejects_garbage() {
        let err = GhostId::from_bech32("not-a-bech32-string-at-all").unwrap_err();
        assert!(matches!(err, GhostIdParseError::InvalidBech32(_)));
    }

    #[test]
    fn fromstr_works_via_display() {
        let id = GhostId::from_bytes([0x42u8; 32]);
        let s = format!("{}", id);
        let parsed: GhostId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    proptest::proptest! {
        #[test]
        fn proptest_roundtrip(bytes in proptest::array::uniform32(proptest::num::u8::ANY)) {
            let id = GhostId::from_bytes(bytes);
            let s = id.to_bech32();
            let back = GhostId::from_bech32(&s).unwrap();
            proptest::prop_assert_eq!(id, back);
        }
    }
}
