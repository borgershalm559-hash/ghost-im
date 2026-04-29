#!/usr/bin/env bash
# Generate a minisign keypair for signing Ghost release artifacts.
#
# Usage:
#   ./scripts/generate-minisign-keypair.sh
#
# This is a ONE-TIME setup. Run once when bootstrapping the repo's release
# infrastructure. Re-running will refuse to overwrite an existing key.
#
# Output:
#   apps/ghost-desktop/minisign.pub  (committed to repo)
#   apps/ghost-desktop/minisign.key  (NOT committed; upload contents to GH
#                                     secret MINISIGN_PRIVATE_KEY manually)
#
# After running:
#   1. cat the .pub file's last line and paste into tauri.conf.json's
#      plugins.updater.pubkey field (Task 3 of Plan 08).
#   2. cat the .key file's contents and paste into the GitHub repo's
#      Settings > Secrets > Actions > New repository secret named
#      MINISIGN_PRIVATE_KEY.
#   3. SECURELY back up the .key file to a password manager (1Password etc).
#   4. DELETE the .key file from the working tree (it's in .gitignore but
#      do not leave plaintext private keys lying around).

set -euo pipefail

OUT_DIR="$(pwd)/apps/ghost-desktop"
PUB="${OUT_DIR}/minisign.pub"
KEY="${OUT_DIR}/minisign.key"

if [[ -f "${PUB}" ]] || [[ -f "${KEY}" ]]; then
  echo "ERROR: existing minisign key files found at:"
  echo "  ${PUB}"
  echo "  ${KEY}"
  echo "Refusing to overwrite. Move them aside or delete them first."
  exit 1
fi

if ! command -v minisign &>/dev/null; then
  echo "ERROR: minisign not found in PATH."
  echo "Install via: winget install jedisct1.minisign  (Windows)"
  echo "             choco install minisign            (Windows alt)"
  echo "             brew install minisign             (macOS)"
  echo "             apt install minisign              (Linux)"
  exit 1
fi

echo "Generating minisign keypair (you will be prompted for a passphrase)..."
echo "Output dir: ${OUT_DIR}"
echo
echo "Recommended: use an empty passphrase (just press Enter twice)."
echo "Reason: GitHub Actions has no way to interactively type a passphrase;"
echo "an empty key passphrase keeps CI secret management simple."
echo

minisign -G -p "${PUB}" -s "${KEY}"

echo
echo "Done. Next steps:"
echo
echo "1. Add the public key to tauri.conf.json:"
echo "   tail -1 ${PUB}"
echo "   → paste into plugins.updater.pubkey"
echo
echo "2. Add the private key to GitHub secrets:"
echo "   cat ${KEY}"
echo "   → upload contents as repo secret MINISIGN_PRIVATE_KEY"
echo
echo "3. Back up the private key to a password manager."
echo
echo "4. After confirming the secret is uploaded:"
echo "   rm ${KEY}"
