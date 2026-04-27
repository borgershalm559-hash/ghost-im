//! Ghost core types, errors, and utilities. No I/O. No crypto operations beyond hashing.

pub mod id;

pub use id::GhostId;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles_and_has_constant_version() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-core");
    }
}
