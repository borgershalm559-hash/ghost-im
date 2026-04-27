//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-identity");
    }
}
