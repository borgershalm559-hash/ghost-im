# Ghost

Anonymous, end-to-end encrypted desktop messenger. Hybrid of Discord and Telegram, no hosting required.

> **Status:** MVP-1 in early development. Not yet usable. See [docs/superpowers/specs/](docs/superpowers/specs/) for design and [docs/superpowers/plans/](docs/superpowers/plans/) for implementation plans.

## Build

```bash
cargo build --workspace
cargo test --workspace -- --test-threads=1
```

`--test-threads=1` is required because `ghost-identity` keystore tests touch the OS keychain and must run sequentially.

## Workspace layout

- `crates/ghost-core` — common types (GhostId, Fingerprint, errors)
- `crates/ghost-identity` — identity keys, encrypted file, OS keystore
- `crates/ghost-identity-cli` — manual smoke-test CLI

More crates land as later plans (`docs/superpowers/plans/`) are implemented.

## License

AGPL-3.0-only.
