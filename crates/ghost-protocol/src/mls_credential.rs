//! Build openmls credentials from a Ghost identity.

use openmls::prelude::{BasicCredential, CredentialWithKey};
use openmls_basic_credential::SignatureKeyPair;
use openmls_traits::{types::SignatureScheme, OpenMlsProvider};

use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};

/// Build a `(Credential, SignatureKeyPair)` pair for use in openmls.
///
/// Constructs a `BasicCredential` from the user's `GhostId` bytes (32 bytes),
/// builds an MLS `SignatureKeyPair` from the `DeviceKey`'s secret/public bytes,
/// registers the `SignatureKeyPair` in the provider's storage so `MlsGroup`
/// and `KeyPackage` operations can reference it, and returns a `CredentialWithKey`
/// ready for `KeyPackage` generation.
pub fn credential_with_key(
    provider: &GhostMlsProvider,
    ik: &IdentityKey,
    dk: &DeviceKey,
) -> Result<(CredentialWithKey, SignatureKeyPair)> {
    let identity_bytes = ik.ghost_id().as_bytes().to_vec();
    let credential = BasicCredential::new(identity_bytes);

    let signature_key = SignatureKeyPair::from_raw(
        SignatureScheme::ED25519,
        dk.secret_bytes().to_vec(),
        dk.public().to_bytes().to_vec(),
    );

    signature_key
        .store(provider.storage())
        .map_err(|e| ProtoError::Mls(format!("store signature key: {e}")))?;

    // signature_key.public() returns &[u8]; SignaturePublicKey implements From<&[u8]>.
    let sig_pub: openmls::prelude::SignaturePublicKey = signature_key.public().into();
    let credential_with_key = CredentialWithKey {
        credential: credential.into(),
        signature_key: sig_pub,
    };

    Ok((credential_with_key, signature_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn build_credential_for_fresh_identity() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let (_cwk, _sig_key) = credential_with_key(&provider, &ik, &dk).expect("build credential");
    }

    #[test]
    fn distinct_identities_produce_credentials_independently() {
        let provider = new_provider();
        let a_ik = IdentityKey::generate();
        let a_dk = DeviceKey::generate(&a_ik);
        let b_ik = IdentityKey::generate();
        let b_dk = DeviceKey::generate(&b_ik);
        let (_a_cwk, _a_sig) = credential_with_key(&provider, &a_ik, &a_dk).unwrap();
        let (_b_cwk, _b_sig) = credential_with_key(&provider, &b_ik, &b_dk).unwrap();
    }
}
