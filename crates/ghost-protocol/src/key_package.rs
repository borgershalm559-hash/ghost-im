//! KeyPackage generation. A KeyPackage is the publishable bundle a contact uses
//! to add you to an MLS group asynchronously.
//!
//! Each call to [`generate_key_package`] produces a fresh one-time KeyPackage.
//! The builder automatically stores the private init key in the provider's
//! storage so that processing a corresponding Welcome message can locate
//! the matching init key.

use crate::mls_credential::credential_with_key;
use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};
use openmls::prelude::{Ciphersuite, KeyPackage};

/// The MLS ciphersuite Ghost uses: X25519 / AES128-GCM / SHA256 / Ed25519.
pub const GHOST_CIPHERSUITE: Ciphersuite =
    Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

/// Generate a fresh [`KeyPackage`] for the given identity and device key.
///
/// Each call produces a distinct one-time KeyPackage; the private init key is
/// automatically registered in `provider`'s storage so that processing the
/// corresponding Welcome message later can locate the matching init key.
pub fn generate_key_package(
    provider: &GhostMlsProvider,
    ik: &IdentityKey,
    dk: &DeviceKey,
) -> Result<KeyPackage> {
    let (cwk, signer) = credential_with_key(provider, ik, dk)?;

    let bundle = KeyPackage::builder()
        .build(GHOST_CIPHERSUITE, provider, &signer, cwk)
        .map_err(|e| ProtoError::Mls(format!("key package build: {e}")))?;

    Ok(bundle.key_package().clone())
}

/// Generate `count` KeyPackages and store their TLS-serialized bytes in `identity.mls_keypackages`.
/// Updates `identity.next_keypackage_id` accordingly. Each KeyPackage's private init key is
/// registered with `provider`'s storage so that processing the corresponding Welcome later works.
///
/// Typically called immediately after `Identity::generate` to populate the publishable bundle.
pub fn populate_initial_keypackages(
    identity: &mut ghost_identity::Identity,
    provider: &GhostMlsProvider,
    count: u32,
) -> Result<()> {
    use openmls::prelude::tls_codec::Serialize as _;

    let ik = &identity.identity_key;
    let dk = &identity.device_key;
    for _ in 0..count {
        let kp = generate_key_package(provider, ik, dk)?;
        let bytes = kp
            .tls_serialize_detached()
            .map_err(|e| ProtoError::Mls(format!("serialize key package: {e}")))?;
        identity.mls_keypackages.push(bytes);
        identity.next_keypackage_id += 1;
    }
    Ok(())
}

#[cfg(test)]
mod populate_tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn populate_adds_correct_count() {
        let provider = new_provider();
        let mut id = ghost_identity::Identity::generate(Some("Alice".into()), 0);
        populate_initial_keypackages(&mut id, &provider, 5).unwrap();
        assert_eq!(id.mls_keypackages.len(), 5);
        assert_eq!(id.next_keypackage_id, 5);
    }

    #[test]
    fn populate_is_additive() {
        let provider = new_provider();
        let mut id = ghost_identity::Identity::generate(None, 0);
        populate_initial_keypackages(&mut id, &provider, 3).unwrap();
        populate_initial_keypackages(&mut id, &provider, 2).unwrap();
        assert_eq!(id.mls_keypackages.len(), 5);
        assert_eq!(id.next_keypackage_id, 5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn generate_key_package_succeeds_for_fresh_identity() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let kp = generate_key_package(&provider, &ik, &dk).unwrap();
        assert_eq!(kp.ciphersuite(), GHOST_CIPHERSUITE);
    }

    #[test]
    fn each_key_package_has_distinct_init_key() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let kp1 = generate_key_package(&provider, &ik, &dk).unwrap();
        let kp2 = generate_key_package(&provider, &ik, &dk).unwrap();
        // The HPKE init keys MUST differ between two KeyPackages.
        assert_ne!(
            kp1.hpke_init_key().as_slice(),
            kp2.hpke_init_key().as_slice()
        );
    }
}
