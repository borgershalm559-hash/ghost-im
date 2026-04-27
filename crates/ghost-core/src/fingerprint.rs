//! Short fingerprint of a GhostId — for verbal/visual verification.
//! Format: 4 hex groups of 4 chars: "1a2b-3c4d-5e6f-7890" (8 bytes from BLAKE3).

use crate::id::GhostId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 8]);

impl Fingerprint {
    pub fn of(id: &GhostId) -> Self {
        let hash = blake3::hash(id.as_bytes());
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hash.as_bytes()[..8]);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex = hex::encode(self.0);
        write!(
            f,
            "{}-{}-{}-{}",
            &hex[0..4],
            &hex[4..8],
            &hex[8..12],
            &hex[12..16]
        )
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fingerprint({})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_format_matches_pattern() {
        let id = GhostId::from_bytes([0u8; 32]);
        let fp = Fingerprint::of(&id);
        let s = fp.to_string();
        assert_eq!(
            s.len(),
            19,
            "expected 4*4 hex + 3 dashes = 19 chars, got {:?}",
            s
        );
        let groups: Vec<&str> = s.split('-').collect();
        assert_eq!(groups.len(), 4);
        for g in groups {
            assert_eq!(g.len(), 4);
            assert!(g.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn fingerprint_deterministic_per_id() {
        let id = GhostId::from_bytes([7u8; 32]);
        let fp1 = Fingerprint::of(&id);
        let fp2 = Fingerprint::of(&id);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.to_string(), fp2.to_string());
    }

    #[test]
    fn different_ids_produce_different_fingerprints() {
        let a = Fingerprint::of(&GhostId::from_bytes([1u8; 32]));
        let b = Fingerprint::of(&GhostId::from_bytes([2u8; 32]));
        assert_ne!(a, b);
    }

    #[test]
    fn known_vector_zero_id() {
        // Lock the BLAKE3 of all-zero 32 bytes — first 8 bytes hex.
        // Computed: blake3([0u8;32]) → 2ada83c1819a5372... (verified with blake3 v1.8.5)
        let id = GhostId::from_bytes([0u8; 32]);
        let fp = Fingerprint::of(&id);
        assert_eq!(fp.to_string(), "2ada-83c1-819a-5372");
    }
}
