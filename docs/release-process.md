# Ghost release process

This runbook walks through cutting a new Ghost desktop release. The first time
you go through it, do the **One-Time Setup** section. After that, every release
follows the **Cutting a Release** section.

## One-Time Setup

### 1. Generate a minisign keypair

The keypair signs every release artifact. The public half is committed to the
repo and embedded in the binary. The private half is uploaded to GitHub Actions
as a secret and never leaves your machine in plaintext.

```bash
./scripts/generate-minisign-keypair.sh
```

The script asks for a passphrase. **Recommended: use an empty passphrase.**
Reason: GitHub Actions has no way to interactively type a passphrase; if you
use a non-empty one, you must also store it as a separate secret
`MINISIGN_PASSWORD`, which adds a second key to manage. An empty passphrase
keeps the threat model the same (the only attacker who could read the key
is one who already has access to the GH Actions runtime, in which case
the passphrase wouldn't help anyway).

**Output:**
- `apps/ghost-desktop/minisign.pub` — committed (Task 1).
- `apps/ghost-desktop/minisign.key` — NOT committed; you upload to GH and back up.

### 2. Embed the public key in `tauri.conf.json`

Run:

```bash
tail -1 apps/ghost-desktop/minisign.pub
```

Copy the output (starts with `RWQ...`). Edit `apps/ghost-desktop/tauri.conf.json`
and replace `RWQ_PASTE_THE_BASE64_LINE_FROM_minisign.pub_HERE` in
`plugins.updater.pubkey` with the actual key.

Commit and push.

### 3. Set the GitHub coordinates

In `apps/ghost-desktop/tauri.conf.json`, replace:

```json
"endpoints": [
  "https://github.com/OWNER_REPLACE_ME/REPO_REPLACE_ME/releases/latest/download/latest.json"
]
```

with your actual GitHub `<owner>/<repo>` (the org or user that owns the repo,
and the repo name itself).

Commit and push.

### 4. Upload the private key as a GH secret

In your repo on GitHub:

1. Go to **Settings → Secrets and variables → Actions → New repository secret**.
2. Name: `MINISIGN_PRIVATE_KEY`.
3. Value: the contents of `apps/ghost-desktop/minisign.key` (cat the file).
4. Click **Add secret**.

If you used a non-empty passphrase in step 1, also add a second secret named
`MINISIGN_PASSWORD` with that passphrase as the value.

### 5. Back up the private key

Save the contents of `minisign.key` to a password manager (1Password, Bitwarden,
KeePass, etc.). Without this backup, if the GH secret is ever lost, you cannot
sign new releases — old binaries with the embedded pubkey will reject any
new key.

### 6. Delete the local plaintext key

```bash
rm apps/ghost-desktop/minisign.key
```

The key now lives only in GH secrets + your password manager.

## Cutting a Release

### 1. Bump the version

Edit `apps/ghost-desktop/tauri.conf.json` and bump the `version` field to the
new semver value (e.g., `0.0.1` → `0.0.2`).

If you also want to bump the workspace version in `Cargo.toml`'s
`[workspace.package]`, do that here.

Commit:

```bash
git add apps/ghost-desktop/tauri.conf.json Cargo.toml
git commit -m "chore: bump version to v0.0.2"
```

### 2. Tag the release

```bash
git tag -a v0.0.2 -m "Release notes go here

Multi-line notes are fine. They are extracted into the GitHub Release body
and into the latest.json manifest's 'notes' field for the in-app banner."

git push origin master --tags
```

The `release.yml` workflow fires on `v*` tag push.

### 3. Watch the workflow

Go to **Actions** in your GitHub repo. The "Release" workflow should be running.
It does:

1. Guards against unfilled placeholders.
2. Compiles the Rust binary (cold cache: ~10 minutes; warm cache: ~3 minutes).
3. Builds the frontend bundle.
4. Bundles into `.msi` via `cargo tauri build`.
5. Signs the `.msi` with `minisign`.
6. Generates `latest.json` from the signature + tag metadata.
7. Creates a GitHub Release with `.msi`, `.msi.sig`, and `latest.json`.

If anything fails, read the step's logs. Common failures:

- **Placeholder guard failed** — you forgot to substitute `OWNER_REPLACE_ME` or
  the pubkey. Fix `tauri.conf.json` and tag a new version.
- **`MINISIGN_PRIVATE_KEY` secret missing** — re-run step 4 of one-time setup.
- **minisign signing failed with "wrong password"** — your local key has a
  non-empty passphrase but `MINISIGN_PASSWORD` secret is empty. Either
  regenerate the key with empty passphrase or set the secret.

### 4. Verify the release

After the workflow succeeds:

1. Visit `https://github.com/<owner>/<repo>/releases/latest`.
2. Confirm three assets are present: `.msi`, `.msi.sig`, `latest.json`.
3. Download `latest.json` and check its contents:
   - `version` matches your tag (without the `v` prefix).
   - `notes` contains your tag annotation.
   - `platforms.windows-x86_64.url` points at the `.msi` in the Release.
   - `platforms.windows-x86_64.signature` is a non-empty base64 string.

### 5. Smoke test the auto-update path

If a previously-released `Ghost.exe` is installed on a test machine:

1. Open the app. Wait up to 1 minute (the on-startup poll runs early).
2. The yellow `↑ Доступна Ghost X.Y.Z` banner should appear at the top.
3. Click "Перезапустить".
4. The banner switches to a download progress indicator.
5. Once the download completes, the MSI installer takes over and the new
   version launches.
6. After restart, confirm the version in the Ghost ID header matches the new
   release.

If the banner never appears: check `%LOCALAPPDATA%/Ghost/logs/` for warn-level
logs about update checks (added in a future plan; for Plan 08 the failure is
silent — open DevTools and check the console).

## Backing out a bad release

If a release goes wrong (corrupt binary, wrong key, etc.) and you have NOT yet
distributed it widely:

1. Delete the GitHub Release and its tag:

   ```bash
   gh release delete v0.0.2 --yes
   git push --delete origin v0.0.2
   git tag -d v0.0.2
   ```

2. Fix the issue.
3. Tag a new patch version (`v0.0.3` — do NOT reuse `v0.0.2`; pre-existing
   downloads of the bad binary won't auto-redirect).

If users already auto-updated to the bad version, they're stuck running it
until you ship a higher-numbered fix. There is no rollback in MVP-1.

## Plan 09 changes (future reference)

- Adds Win EV code-signing certificate (~$300/yr) — eliminates SmartScreen
  warning at first run.
- Adds macOS notarization + `.dmg` bundles (Apple Dev account, $99/yr).
- N-of-M signing (3 minisign keys, 2 of 3 must sign each release).
- Sigstore Rekor transparency log entries.
- 100% reproducible builds via dockerized CI.
- Settings UI for auto-update preferences.
- Inline changelog in the banner.
