//! MLS provider — bundles crypto + storage + rand for openmls.
//!
//! For Plan 02 we use the in-memory provider from `openmls_rust_crypto`.
//! Plan 03 (storage) will replace the storage half with SQLite-backed persistence.

use openmls_rust_crypto::OpenMlsRustCrypto;

/// Type alias so callers don't have to import openmls_rust_crypto directly.
pub type GhostMlsProvider = OpenMlsRustCrypto;

/// Construct a fresh in-memory provider. Each session/test should get its own.
pub fn new_provider() -> GhostMlsProvider {
    OpenMlsRustCrypto::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmls_traits::OpenMlsProvider;

    #[test]
    fn provider_constructs_and_exposes_traits() {
        let provider = new_provider();
        // Just touch the three trait methods to make sure the type implements them.
        let _ = provider.crypto();
        let _ = provider.storage();
        let _ = provider.rand();
    }
}
