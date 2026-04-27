#!/usr/bin/env bash
# End-to-end smoke test for Plan 01 deliverable.
# Verifies: create → show → wipe → show-fails — full identity lifecycle.

set -euo pipefail

# On Windows the default cargo invocation can fail with "dlltool not found" due
# to PATH ordering. Pin the toolchain explicitly so all cargo calls in this
# script use the msvc linker chain unambiguously.
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.85-x86_64-pc-windows-msvc}"

SMOKE_HOME="${TMPDIR:-/tmp}/ghost-smoke-plan01-$$"
trap 'rm -rf "$SMOKE_HOME"' EXIT

export GHOST_HOME="$SMOKE_HOME"
mkdir -p "$SMOKE_HOME"

echo "==> 1. Create identity"
CREATE_OUT=$(cargo run --quiet -p ghost-identity-cli -- create --display-name "SmokeAlice" --passphrase "test-pp")
echo "$CREATE_OUT"
CREATED_ID=$(echo "$CREATE_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ -n "$CREATED_ID" ] || { echo "FAIL: no Ghost ID in create output"; exit 1; }
echo "Got Ghost ID: $CREATED_ID"

echo "==> 2. Verify file exists and is encrypted (no plaintext leak)"
test -f "$SMOKE_HOME/identity.encrypted" || { echo "FAIL: file missing"; exit 1; }
if grep -aq "SmokeAlice" "$SMOKE_HOME/identity.encrypted"; then
  echo "FAIL: display name leaked in plaintext"
  exit 1
fi
echo "OK — file present, no plaintext leak"

echo "==> 3. Show identity, verify same Ghost ID"
SHOW_OUT=$(cargo run --quiet -p ghost-identity-cli -- show --passphrase "test-pp")
echo "$SHOW_OUT"
SHOWN_ID=$(echo "$SHOW_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ "$SHOWN_ID" = "$CREATED_ID" ] || { echo "FAIL: show returned different ID"; exit 1; }
echo "$SHOW_OUT" | grep -q "DK signature: valid" || { echo "FAIL: DK signature invalid"; exit 1; }
echo "OK — identity round-trips correctly"

echo "==> 4. Show with wrong passphrase MUST fail"
if cargo run --quiet -p ghost-identity-cli -- show --passphrase "wrong-pp" 2>/dev/null; then
  echo "FAIL: wrong passphrase was accepted"
  exit 1
fi
echo "OK — wrong passphrase rejected"

echo "==> 5. Wipe and verify"
cargo run --quiet -p ghost-identity-cli -- wipe --yes
test ! -f "$SMOKE_HOME/identity.encrypted" || { echo "FAIL: file still present after wipe"; exit 1; }
echo "OK — wipe removed identity"

echo "==> 6. Show after wipe MUST fail with NotFound"
if cargo run --quiet -p ghost-identity-cli -- show 2>/dev/null; then
  echo "FAIL: show succeeded after wipe"
  exit 1
fi
echo "OK — show rejects missing identity"

echo "==> 7. New identity after wipe has different Ghost ID"
NEW_OUT=$(cargo run --quiet -p ghost-identity-cli -- create --display-name "SmokeAlice2")
NEW_ID=$(echo "$NEW_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ "$NEW_ID" != "$CREATED_ID" ] || { echo "FAIL: regenerated identity has same ID"; exit 1; }
echo "OK — new identity is genuinely fresh"

cargo run --quiet -p ghost-identity-cli -- wipe --yes

echo
echo "==> Plan 01 smoke test PASSED"
