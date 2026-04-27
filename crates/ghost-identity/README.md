# ghost-identity

Identity primitives: Ed25519 IK/DK, encrypted identity file, OS keystore.

## Testing notes

Tests in `keystore` and `identity::create_load_tests` modules touch the **real OS keystore**.
Run with `--test-threads=1`:

```bash
cargo test -p ghost-identity -- --test-threads=1
```

### Headless Linux (CI)

The keyring backend on Linux requires a session DBus + Secret Service. On headless CI:

- Install `gnome-keyring` and `dbus-x11`.
- Wrap tests in `dbus-launch --exit-with-session` and `gnome-keyring-daemon --start --components=secrets`.

If your CI cannot provide that, skip the keystore tests:

```bash
cargo test -p ghost-identity -- --test-threads=1 --skip keystore::tests --skip identity::create_load_tests
```

…and verify them manually on developer machines for each platform before release.

## File format

`identity.encrypted` layout (see `file_format.rs`):

```
[0..4]    magic "GHST"
[4]       version u8 (currently 1)
[5..21]   16-byte Argon2id salt
[21..45]  24-byte XChaCha20 nonce
[45..]    XChaCha20-Poly1305 ciphertext (CBOR-serialized Identity) + 16-byte tag
```

The AEAD AAD is the literal `b"ghost.identity.v1.aad"`. Tampering with the header makes decryption fail.
