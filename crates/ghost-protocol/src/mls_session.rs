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

/// Result of inviting a new member: the Welcome (to be sent to the invitee out-of-band)
/// plus the Commit (which the inviter applies locally — handled inside this method).
pub struct InviteResult {
    pub welcome: MlsMessageOut,
    pub commit: MlsMessageOut,
}

impl MlsSession {
    /// Invite a new member by their KeyPackage. Produces a Welcome the invitee can use to join,
    /// and a Commit message that — in larger groups — would be broadcast to other members.
    /// In our 1-on-1 case there are no other members yet, so the Commit is mostly bookkeeping.
    ///
    /// openmls 0.8 `add_members` returns `(commit, welcome, Option<GroupInfo>)`.
    pub fn add_member(
        &mut self,
        provider: &GhostMlsProvider,
        signer: &SignatureKeyPair,
        invitee_kp: KeyPackage,
    ) -> Result<InviteResult> {
        let (commit, welcome, _group_info) = self
            .group
            .add_members(provider, signer, &[invitee_kp])
            .map_err(|e| ProtoError::Mls(format!("add member: {e}")))?;

        // Apply the membership change locally so our state advances to epoch 1.
        self.group
            .merge_pending_commit(provider)
            .map_err(|e| ProtoError::Mls(format!("merge commit: {e}")))?;

        Ok(InviteResult { welcome, commit })
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

#[cfg(test)]
mod add_member_tests {
    use super::*;
    use crate::key_package::generate_key_package;
    use crate::mls_provider::new_provider;

    #[test]
    fn add_member_advances_epoch() {
        let alice_provider = new_provider();
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let mut alice = MlsSession::create(&alice_provider, &alice_ik, &alice_dk).unwrap();
        assert_eq!(alice.epoch(), 0);

        // Bob (separate provider — represents a separate process/machine).
        let bob_provider = new_provider();
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        let bob_kp = generate_key_package(&bob_provider, &bob_ik, &bob_dk).unwrap();

        let alice_signer = MlsSession::signer_from_dk(&alice_dk);
        let invite = alice
            .add_member(&alice_provider, &alice_signer, bob_kp)
            .unwrap();
        assert_eq!(alice.epoch(), 1);

        let _ = invite.welcome;
        let _ = invite.commit;
    }
}
