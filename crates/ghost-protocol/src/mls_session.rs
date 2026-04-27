//! MlsSession — domain wrapper around openmls's MlsGroup, scoped to 1-on-1 conversations.

use crate::key_package::GHOST_CIPHERSUITE;
use crate::mls_credential::credential_with_key;
use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_traits::types::SignatureScheme;

pub struct MlsSession {
    group: MlsGroup,
}

impl MlsSession {
    /// Create a fresh MLS group containing only the creator. After this, call
    /// [`add_member`] to invite the conversation partner.
    pub fn create(
        provider: &GhostMlsProvider,
        ik: &IdentityKey,
        dk: &DeviceKey,
    ) -> Result<Self> {
        let (cwk, signer) = credential_with_key(provider, ik, dk)?;

        let group_config = MlsGroupCreateConfig::builder()
            .ciphersuite(GHOST_CIPHERSUITE)
            .use_ratchet_tree_extension(true)
            .build();

        let group = MlsGroup::new(provider, &signer, &group_config, cwk)
            .map_err(|e| ProtoError::Mls(format!("create group: {e}")))?;

        Ok(Self { group })
    }

    pub fn epoch(&self) -> u64 {
        self.group.epoch().as_u64()
    }

    /// Build a fresh signer for the given DK — convenience for tests and downstream callers
    /// that need a `SignatureKeyPair` reference but don't have one stored.
    pub fn signer_from_dk(dk: &DeviceKey) -> SignatureKeyPair {
        SignatureKeyPair::from_raw(
            SignatureScheme::ED25519,
            dk.secret_bytes().to_vec(),
            dk.public().to_bytes().to_vec(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn create_session_starts_at_epoch_zero() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let session = MlsSession::create(&provider, &ik, &dk).unwrap();
        assert_eq!(session.epoch(), 0);
    }
}
