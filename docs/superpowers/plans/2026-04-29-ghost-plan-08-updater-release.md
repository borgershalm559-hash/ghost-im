# Ghost Plan 08 — Updater + Release Pipeline

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Working end-to-end auto-update for Windows desktop binaries via GitHub Releases. Old version finds + downloads + verifies + installs new version. Manifest signed with offline minisign key. No code signing certs purchased (SmartScreen warning ack'd; Plan 09 adds EV cert).

**Architecture:** `tauri-plugin-updater` v2 integrated in `apps/ghost-desktop/`. Manifest (`latest.json`) lives as a GitHub Release asset; updater polls `https://github.com/<owner>/<repo>/releases/latest/download/latest.json` (GitHub's redirect-to-latest mechanism). Public minisign key embedded in binary at build time via `tauri.conf.json`; private key kept as GitHub Actions secret. Frontend `UpdateBanner.svelte` shows toast with "Перезапустить" / "Позже" when an update is found. CI: two GitHub Actions workflows (CI on push/PR, Release on `v*` tags). Reproducible build flags applied via `.cargo/config.toml` and `SOURCE_DATE_EPOCH`.

**Tech Stack:** Existing Rust + Tauri 2.10 + SvelteKit stack. New deps: `tauri-plugin-updater = "2"` (Rust), `@tauri-apps/plugin-updater = "^2"` (frontend). External tool: `minisign` (signature CLI, available via chocolatey or pre-built binary). CI: GitHub Actions on `windows-latest` (release) + `ubuntu-latest` (frontend lint).

**Deliverable Plan 08:** A signed `.msi` and `latest.json` published to GitHub Releases on every `v*` tag push. Old binaries find the new version, verify the signature, download, and install — all driven by the user clicking "Перезапустить" in the toast. End-to-end smoke verified manually by cutting two adjacent versions in a test repo. No automated UI test (out of scope for MVP-1, same as Plan 07).

A short list of what is **NOT** in Plan 08 (deferred to Plan 09 or later):
- Windows EV Code Signing Certificate (~$300/yr).
- macOS Apple Developer Program ($99/yr) + notarization.
- Linux builds (per user decision: Windows-only in MVP-1).
- N-of-M signing (2 of 3 keys, YubiKey backup).
- Sigstore Rekor transparency log entries.
- 100% reproducible builds via docker. MVP-1: ~80% via flags.
- Custom domain / Cloudflare Pages for update channel.
- Settings UI (auto-download / notify / disabled toggle, polling cadence).
- Inline changelog in banner.
- "Remind me later" timer beyond current session.
- `min_supported` field in manifest.
- Update kill-switch / revocation list.
- Custom NSIS/WiX installer themes.
- Landing page with download buttons.

**Reference spec:** [docs/superpowers/specs/2026-04-29-ghost-mvp1-plan-08-design.md](../specs/2026-04-29-ghost-mvp1-plan-08-design.md)

**Reference plans:**
- [Plan 06](2026-04-28-ghost-plan-06-client-orchestration.md) — `Client::open` + reactive store patterns.
- [Plan 07](2026-04-28-ghost-plan-07-tauri-app.md) — Tauri shell binary, frontend reactive store, capability config.

---

## Notes for the implementer

**Tauri 2.x updater plugin docs:** https://v2.tauri.app/plugin/updater/. Read the "Configuration" and "Frontend" sections before starting Task 3 — the API surface is small but the manifest-format expectations are specific.

**`tauri-plugin-updater` works with two artifacts per release:**
1. `latest.json` — manifest pointing at the platform-specific binaries plus their signatures.
2. `<binary>.msi.sig` — minisign signature file produced by `minisign -S`.

The plugin downloads `latest.json` from the configured `endpoints[]`, then for the running platform downloads the binary URL named in `platforms.<target>.url`, and verifies its signature using the embedded `pubkey`. On Windows the `target` key is `windows-x86_64`.

**Endpoint URL choice:** Use `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`. GitHub redirects this to the actual latest release's `latest.json` asset, so the URL never changes between releases. This is the official Tauri-recommended pattern for GitHub-hosted updates.

**`<owner>/<repo>` placeholder:** The implementer does NOT decide this. Until the user picks a GitHub org, leave the literal string `OWNER_REPLACE_ME/REPO_REPLACE_ME` and document the substitution in `docs/release-process.md`. Add a CI guard in the release workflow that fails the build if the placeholder is still present in `tauri.conf.json` at release time (Task 11).

**`minisign` install on Windows:** The CI runner uses `choco install minisign` (chocolatey). For local development the implementer runs the same. minisign is a tiny self-contained CLI by Frank Denis (jedisct1) implementing the same scheme as OpenBSD's signify. NOT to be confused with `minisig` or `minisignature`.

**Keypair generation:** Task 1's `scripts/generate-minisign-keypair.sh` calls `minisign -G` and writes `minisign.pub` (committed to repo) and `minisign.key` (NOT committed; user uploads contents to GH secret manually). The script is bash; on Windows it runs in Git Bash.

**Why use a `.sh` and not PowerShell:** the user runs Git Bash for development; matches existing `scripts/` convention (e.g., `scripts/launch-ghost-desktop.sh`).

**Pubkey embedding:** `minisign.pub` is a 2-line file:
```
untrusted comment: minisign public key XXXXX
RWQ...base64...XXXXX
```
The pubkey field in `tauri.conf.json` wants ONLY the base64 line (the `RWQ...` part), NOT the comment. Task 3 instructs the implementer to extract `tail -1` of the file.

**`@tauri-apps/plugin-updater` JS API (v2):**
```ts
import { check } from '@tauri-apps/plugin-updater';
const update = await check(); // Update | null
if (update) {
  await update.downloadAndInstall((event) => {
    // event.event === 'Started' | 'Progress' | 'Finished'
  });
}
```

**Version comparison semantics:** `tauri-plugin-updater` uses semver. Pre-release tags (`-alpha.1`, `-rc.1`) are handled. We commit to semver-compatible tags only (`v0.0.1`, `v0.1.0`, etc.).

**Update polling cadence:** The plugin does NOT auto-poll. We trigger checks ourselves: once at app startup (in the UpdateBanner `onMount`), and every hour via `setInterval` in the same component. No background timer in Rust.

**Why the auto-poll is in the frontend not Rust:** simpler. The component is alive whenever the window is open; if the window is minimized the user doesn't care about update toasts anyway.

**Real icon in MVP-1:** Geometric ghost silhouette, white-on-transparent. Source PNG 1024×1024 in `apps/ghost-desktop/icons/icon.png`. All other sizes auto-generated via `cargo tauri icon icons/icon.png`. The implementer creates the source PNG using any vector/raster tool — no specific tool required. If the implementer cannot draw, a 5-minute SVG-to-PNG with two overlapping circles + a wavy bottom is acceptable; we will replace with a real designed icon in a future plan.

**CI matrix in Plan 08:** Windows-only for `release.yml`. `ci.yml` adds Ubuntu for the frontend lint job (faster + free) and Windows for the Rust check job. macOS jobs are NOT added; Plan 09 will introduce them.

**Reproducible builds at 80%:** `--remap-path-prefix` and `SOURCE_DATE_EPOCH` cover most non-determinism. Remaining 20% (Rust compiler internal randomness, native dep timestamps) is acceptable for MVP-1; community can still verify "shipped bytes ≈ source code." Plan 09 hardens to 100% via docker-based builds.

**Manifest signing strategy:** Only the `.msi` is signed (signature embedded in `latest.json` as `platforms.windows-x86_64.signature`). The manifest itself is NOT signed. This is the official `tauri-plugin-updater` pattern; manifest tampering only enables denial-of-update, not malicious update (the inner signature stays valid against bytes the attacker can't forge).

**Test isolation for the updater code:** The plugin's network calls cannot be unit-tested without a fake HTTP server. Plan 08 does NOT add such infrastructure; the only test is the manual end-to-end smoke in Task 13. Acceptable for MVP-1; Plan 09 will add a Wiremock-based test harness if it makes sense at that point.

**No new Rust unit tests in this plan.** The updater commands are thin wrappers. Same reasoning as Plan 07.

**Frontend updates:** `UpdateBanner.svelte` is small enough that a Vitest integration test would be more setup than value. Skipped.

**File-size discipline:** Same as Plan 07 — no Rust file past ~200 lines, no `.svelte` past ~150 lines.

---

## Task 1: Generate minisign keypair + commit pubkey

**Files:**
- Create: `scripts/generate-minisign-keypair.sh`
- Create: `apps/ghost-desktop/minisign.pub` (manually generated; output of script)

This task is partly manual: `minisign -G` is interactive (asks for password). The script wraps the call but the user must run it locally. The output file `minisign.pub` is then committed; the corresponding `minisign.key` is NOT committed and must be uploaded manually as a GitHub secret later.

### Step 1.1: Verify minisign is available

- [ ] **Run:** `which minisign` (or `where minisign` on Windows)
- [ ] **Expected:** Path to the binary printed.
- [ ] If missing on Windows: `choco install minisign -y` (requires admin shell). On Mac: `brew install minisign`. On Ubuntu: `apt install minisign`.

### Step 1.2: Create `scripts/generate-minisign-keypair.sh`

- [ ] **Create file** with content:

```bash
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
  echo "Install via: choco install minisign  (Windows)"
  echo "             brew install minisign   (macOS)"
  echo "             apt install minisign    (Linux)"
  exit 1
fi

echo "Generating minisign keypair (you will be prompted for a passphrase)..."
echo "Output dir: ${OUT_DIR}"
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
```

- [ ] **Make executable:** `chmod +x scripts/generate-minisign-keypair.sh`

### Step 1.3: Run the script

- [ ] **Run:** `./scripts/generate-minisign-keypair.sh`
- [ ] **Interaction:** minisign asks for a passphrase. Use a strong one; record it in your password manager. The passphrase protects the key against on-disk theft (GH secret upload removes the protection in CI but a copy on the dev machine should stay encrypted).
- [ ] **Expected output:** Two new files at `apps/ghost-desktop/{minisign.pub,minisign.key}`.

### Step 1.4: Add `minisign.key` to `.gitignore`

- [ ] **Edit `apps/ghost-desktop/.gitignore`** (create if missing) — add:

```
# Minisign private key — never commit. Generated by
# scripts/generate-minisign-keypair.sh and uploaded as GH secret manually.
minisign.key
```

If `.gitignore` doesn't exist in `apps/ghost-desktop/`, create it. (Plan 07 didn't add one; the only contents need to be the line above.)

### Step 1.5: Verify the public key file is correct

- [ ] **Run:** `cat apps/ghost-desktop/minisign.pub`
- [ ] **Expected:** Two lines:
  - Line 1: `untrusted comment: minisign public key <hex-id>`
  - Line 2: `RWQ...<long base64 string>...`

### Step 1.6: Commit (public key + script + gitignore)

- [ ] **Run:**

```bash
git add scripts/generate-minisign-keypair.sh apps/ghost-desktop/minisign.pub apps/ghost-desktop/.gitignore
git status
```

Verify ONLY those three files are staged. **`minisign.key` MUST NOT appear in git status; if it does, your `.gitignore` line is wrong — fix before committing.**

- [ ] **Run:**

```bash
git commit -m "feat(release): add minisign keypair generation helper + commit public key"
```

### Step 1.7: Final manual cleanup

After the commit lands and you've uploaded the key contents to a password manager:

- [ ] **Run:** `rm apps/ghost-desktop/minisign.key`

The key file lives in CI as `MINISIGN_PRIVATE_KEY` secret AND in your password manager. The repo working tree should not have it.

(For testing local builds before Tasks 11-13 wire up GH Actions, you may temporarily restore the key from your password manager — just don't commit it.)

---

## Task 2: Real Windows icons (replace 66B placeholder)

**Files:**
- Create: `apps/ghost-desktop/icons/icon.png` (1024×1024 source)
- Replace: `apps/ghost-desktop/icons/icon.ico` (auto-generated from source)
- Create: `apps/ghost-desktop/icons/{32x32,128x128,128x128@2x,Square*}.png` (auto-generated)

### Step 2.1: Create the source PNG (1024×1024)

The icon must be a 1024×1024 transparent PNG. Content: a geometric ghost silhouette in white. Plan 08 doesn't lock the aesthetic; the implementer chooses any tool (Inkscape, GIMP, Photoshop, Figma export, online SVG-to-PNG converter). What matters is:

- Square format
- Transparent background
- Subject visible at 16×16 (avoid thin lines)
- White or near-white fill (looks fine on dark Tauri window bar)

A minimal acceptable icon: take the unicode ghost emoji 👻, render it onto a transparent 1024×1024 canvas in white. Tools like Pillow (Python) can do this in 4 lines:

```python
from PIL import Image, ImageDraw, ImageFont
img = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
draw = ImageDraw.Draw(img)
draw.text((128, 32), '👻', fill='white',
          font=ImageFont.truetype('seguiemj.ttf', 800))
img.save('apps/ghost-desktop/icons/icon.png')
```

Or any equivalent. **The implementer may NOT use the 66-byte placeholder ICO as the source — that file is 1×1 pixel.**

- [ ] **Create file `apps/ghost-desktop/icons/icon.png`** by whatever method, as long as it's 1024×1024 RGBA PNG with white-on-transparent ghost silhouette.

- [ ] **Verify with:** `python -c "from PIL import Image; im=Image.open('apps/ghost-desktop/icons/icon.png'); print(im.size, im.mode)"`
- [ ] **Expected:** `(1024, 1024) RGBA`

If the implementer doesn't have Python/PIL, ImageMagick works: `magick identify apps/ghost-desktop/icons/icon.png` should print `... PNG 1024x1024 ...`.

### Step 2.2: Generate the multi-size icons via Tauri CLI

- [ ] **Run** from repo root:

```bash
cargo +1.87-x86_64-pc-windows-msvc tauri icon apps/ghost-desktop/icons/icon.png \
  --output apps/ghost-desktop/icons
```

- [ ] **Expected:** Output `Wrote ... .png/.ico/.icns` for ~10 sized variants. The placeholder `icon.ico` is overwritten with a real multi-resolution `.ico` containing 16/32/48/64/128/256.

### Step 2.3: Verify the new ICO is valid

- [ ] **Run:** `file apps/ghost-desktop/icons/icon.ico` (Linux/macOS/Git-Bash)
  - **Expected:** `MS Windows icon resource - <N> icons, <sizes>`
  - The new file should report multiple icons across the 16-256 range, NOT just `1 icon, 1x1`.
- [ ] **Run:** `ls -la apps/ghost-desktop/icons/icon.ico`
  - **Expected:** Size > 10 KB (a real multi-size ICO is at least ~30 KB; the 66B placeholder is gone).

### Step 2.4: Update `tauri.conf.json` icon list

The current `tauri.conf.json` references only `icons/icon.ico`. Tauri's `cargo tauri icon` produces a longer list; update the config:

- [ ] **Edit `apps/ghost-desktop/tauri.conf.json`** — change the `bundle.icon` array:

From:
```json
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.ico"]
  }
```

To:
```json
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
```

(`cargo tauri icon` always generates these names; the `icon.icns` is the macOS bundle icon — harmless on Windows.)

### Step 2.5: Verify build still succeeds

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc check -p ghost-desktop
```

- [ ] **Expected:** `Finished` cleanly. Tauri-build at compile time embeds the icon resources; if the file list is wrong, this fails immediately.

### Step 2.6: Commit

- [ ] **Run:**

```bash
git add apps/ghost-desktop/icons/ apps/ghost-desktop/tauri.conf.json
git commit -m "feat(ghost-desktop): real multi-size Windows icon, replacing 1x1 placeholder"
```

---

## Task 3: Wire `tauri-plugin-updater` (dep + plugin + config + capabilities)

This task wires the updater plugin end-to-end at the configuration level. No commands yet (Task 5) and no UI (Tasks 7-8) — just the plumbing so the next tasks can build on a registered plugin.

**Files:**
- Modify: `apps/ghost-desktop/Cargo.toml`
- Modify: `apps/ghost-desktop/src/main.rs`
- Modify: `apps/ghost-desktop/tauri.conf.json`
- Modify: `apps/ghost-desktop/capabilities/default.json`

### Step 3.1: Add the Rust dep

- [ ] **Edit `apps/ghost-desktop/Cargo.toml`** — add to `[dependencies]`:

```toml
tauri-plugin-updater = { version = "2", features = ["native-tls"] }
```

The `native-tls` feature uses Windows' built-in schannel (no openssl dep at runtime). Plan 09 may switch to `rustls` if cross-platform consistency matters more.

### Step 3.2: Register the plugin in `main.rs`

- [ ] **Edit `apps/ghost-desktop/src/main.rs`** — add `.plugin()` call to the `Builder::default()` chain.

The current `main()` body looks like:

```rust
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // ...10 commands...
        ])
        .setup(|app| { /* ... */ })
        .run(tauri::generate_context!())
        .expect("ghost-desktop failed to run");
```

Insert the plugin registration BEFORE `.manage(...)`:

```rust
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // ...10 commands...  (unchanged in Task 3)
        ])
        .setup(|app| { /* ... */ })
        .run(tauri::generate_context!())
        .expect("ghost-desktop failed to run");
```

### Step 3.3: Configure the plugin in `tauri.conf.json`

The plugin reads its config from `tauri.conf.json` under the `plugins.updater` key.

First, get the pubkey value from `apps/ghost-desktop/minisign.pub` (created in Task 1):

- [ ] **Run:** `tail -1 apps/ghost-desktop/minisign.pub`
- [ ] **Expected:** A single line starting with `RWQ...` (or similar minisign prefix).

Copy that line. Now:

- [ ] **Edit `apps/ghost-desktop/tauri.conf.json`** — add a top-level `plugins` key with the updater config:

The full file should now look like:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Ghost",
  "version": "0.0.1",
  "identifier": "im.ghost.desktop",
  "build": {
    "frontendDist": "../../frontend/build",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": {
      "cwd": "../../frontend",
      "script": "pnpm dev"
    },
    "beforeBuildCommand": {
      "cwd": "../../frontend",
      "script": "pnpm build"
    }
  },
  "app": {
    "windows": [
      {
        "title": "Ghost",
        "width": 1024,
        "height": 720,
        "minWidth": 480,
        "minHeight": 480,
        "resizable": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost https://github.com https://api.github.com https://*.githubusercontent.com"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  },
  "plugins": {
    "updater": {
      "active": true,
      "dialog": false,
      "endpoints": [
        "https://github.com/OWNER_REPLACE_ME/REPO_REPLACE_ME/releases/latest/download/latest.json"
      ],
      "pubkey": "RWQ_PASTE_THE_BASE64_LINE_FROM_minisign.pub_HERE"
    }
  }
}
```

Replace:
- `OWNER_REPLACE_ME/REPO_REPLACE_ME` — left as a literal placeholder. `docs/release-process.md` (Task 12) will tell the user this is the one thing to substitute when first publishing. The release workflow (Task 11) adds a CI guard that fails the build if the placeholder is still present.
- `RWQ_PASTE_THE_BASE64_LINE_FROM_minisign.pub_HERE` — paste the actual pubkey line you copied earlier.

The CSP `connect-src` was extended with `https://github.com https://api.github.com https://*.githubusercontent.com` so the updater can fetch from those domains. Without it the WebView blocks the requests.

`"dialog": false` tells the plugin NOT to show its built-in modal dialog when an update is found — we render our own UpdateBanner (Tasks 7-8).

### Step 3.4: Add updater permissions to `capabilities/default.json`

- [ ] **Edit `apps/ghost-desktop/capabilities/default.json`** — append to the `permissions` array:

```json
    "updater:default",
    "updater:allow-check",
    "updater:allow-download-and-install"
```

The full file should now look like:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "identifier": "default",
  "description": "Allow main window to invoke ghost-app commands and receive inbox events.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "updater:default",
    "updater:allow-check",
    "updater:allow-download-and-install"
  ]
}
```

### Step 3.5: Verify compilation

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc check -p ghost-desktop
```

- [ ] **Expected:** First compile pulls `tauri-plugin-updater` and its deps (~30-60 seconds). Then `Finished`. Any compilation error here means the plugin version doesn't match Tauri 2.10.x — verify both are on `2.x` major.

### Step 3.6: Commit

- [ ] **Run:**

```bash
git add apps/ghost-desktop/Cargo.toml apps/ghost-desktop/src/main.rs apps/ghost-desktop/tauri.conf.json apps/ghost-desktop/capabilities/default.json Cargo.lock
git commit -m "feat(ghost-desktop): wire tauri-plugin-updater (dep + plugin + config + capabilities)"
```

---

## Task 4: Add `UpdateAvailableDto` to Rust + frontend types

**Files:**
- Modify: `crates/ghost-app/src/dto.rs`
- Modify: `frontend/src/lib/types.ts`

### Step 4.1: Add the Rust DTO

- [ ] **Edit `crates/ghost-app/src/dto.rs`** — add this struct (anywhere in the file; alphabetical order after the existing `MessageDto` is reasonable):

```rust
/// Result of `check_for_update` command. `None` (in `CommandResult<Option<…>>`) means
/// no update available; `Some(...)` means an update is available with these details.
#[derive(Debug, Serialize)]
pub struct UpdateAvailableDto {
    pub version: String,
    pub notes: Option<String>,
    pub release_date: Option<String>,
}
```

`release_date` is RFC3339 string (whatever `tauri-plugin-updater` returns), kept as String to keep the DTO frontend-friendly.

### Step 4.2: Add the matching TypeScript type

- [ ] **Edit `frontend/src/lib/types.ts`** — append:

```ts
export interface UpdateAvailableDto {
  version: string;
  notes: string | null;
  release_date: string | null;
}
```

### Step 4.3: Verify both sides compile

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app
pnpm --dir frontend check
```

- [ ] **Expected:** Both clean. `pnpm check` reports 0 errors.

### Step 4.4: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/src/dto.rs frontend/src/lib/types.ts
git commit -m "feat(ghost-app): UpdateAvailableDto on both Rust and TS sides"
```

---

## Task 5: Updater Tauri commands (`check_for_update`, `download_and_install_update`)

**Files:**
- Create: `crates/ghost-app/src/commands/updater.rs`
- Modify: `crates/ghost-app/src/commands/mod.rs`
- Modify: `apps/ghost-desktop/src/main.rs` (to register the new commands)

### Step 5.1: Create the Rust command file

- [ ] **Create `crates/ghost-app/src/commands/updater.rs`** with content:

```rust
//! Updater commands: check for available updates, download + install them.
//!
//! These are thin wrappers over `tauri-plugin-updater`'s `UpdaterExt` trait.
//! The actual download progress is tracked via Tauri events emitted by the
//! plugin itself (`tauri-plugin-updater` v2 emits `update-progress` payloads
//! through a callback we pass to `download_and_install`).

use crate::dto::UpdateAvailableDto;
use crate::error::{CommandError, CommandResult};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Polls the configured update endpoints. Returns the update details if one is
/// available, or `None` if the running version is the latest.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> CommandResult<Option<UpdateAvailableDto>> {
    let updater = app
        .updater()
        .map_err(|e| CommandError(format!("updater init: {e}")))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateAvailableDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            release_date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(CommandError(format!("update check: {e}"))),
    }
}

/// Downloads and installs the latest update. Emits `update-progress` events
/// during the download. Returns once the installer has been launched (the
/// current process exits shortly after).
#[tauri::command]
pub async fn download_and_install_update(app: AppHandle) -> CommandResult<()> {
    let updater = app
        .updater()
        .map_err(|e| CommandError(format!("updater init: {e}")))?;
    let update = updater
        .check()
        .await
        .map_err(|e| CommandError(format!("update check: {e}")))?
        .ok_or_else(|| CommandError("no update available".to_string()))?;

    let app_clone = app.clone();
    update
        .download_and_install(
            move |chunk_length, content_length| {
                let payload = ProgressPayload {
                    chunk: chunk_length as u64,
                    total: content_length.unwrap_or(0),
                };
                let _ = app_clone.emit("ghost://update-progress", payload);
            },
            || {
                // Finished — handler called once after install completes.
            },
        )
        .await
        .map_err(|e| CommandError(format!("download/install: {e}")))?;
    Ok(())
}

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    chunk: u64,
    total: u64,
}
```

### Step 5.2: Register the new submodule

- [ ] **Edit `crates/ghost-app/src/commands/mod.rs`** — add `pub mod updater;`. The full file becomes:

```rust
//! Tauri command implementations.

pub mod identity;
pub mod lifecycle;
pub mod read;
pub mod updater;
pub mod write;
```

### Step 5.3: Register the commands in `main.rs`

- [ ] **Edit `apps/ghost-desktop/src/main.rs`** — add `updater` to the `use` line and the two new commands to the `generate_handler!` macro.

Modify the `use` line:

```rust
use ghost_app::commands::{identity, lifecycle, read, updater, write};
```

Modify `invoke_handler`:

```rust
        .invoke_handler(tauri::generate_handler![
            identity::identity_status,
            identity::create_identity,
            lifecycle::open_client,
            lifecycle::close_client,
            read::client_info,
            read::list_contacts,
            read::list_messages,
            read::create_invite,
            updater::check_for_update,
            updater::download_and_install_update,
            write::add_contact,
            write::send_message,
        ])
```

### Step 5.4: Verify compilation

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc check -p ghost-desktop
```

- [ ] **Expected:** `Finished`. The `tauri_plugin_updater::UpdaterExt` trait is in scope via the use statement in updater.rs; if the compiler complains about `app.updater()` returning a different shape, the plugin's API changed between 2.x minors. Read the actual error and adjust accordingly — do NOT add new methods to upstream.

### Step 5.5: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/src/commands/ apps/ghost-desktop/src/main.rs Cargo.lock
git commit -m "feat(ghost-app): updater commands (check_for_update, download_and_install_update)"
```

---

## Task 6: Frontend bridge wrappers + event subscription

**Files:**
- Modify: `frontend/src/lib/tauri.ts`
- Modify: `frontend/package.json` (add @tauri-apps/plugin-updater)

### Step 6.1: Install the JS plugin

- [ ] **Edit `frontend/package.json`** — add to `dependencies`:

```json
    "@tauri-apps/plugin-updater": "^2"
```

(The full block becomes `"@tauri-apps/api": "^2", "@tauri-apps/plugin-updater": "^2"`.)

- [ ] **Run:** `pnpm --dir frontend install`
- [ ] **Expected:** `pnpm-lock.yaml` updated with the new dep.

### Step 6.2: Add wrappers to `tauri.ts`

- [ ] **Edit `frontend/src/lib/tauri.ts`** — append to the imports the new types and constant:

Add after the existing `INBOX_EVENT` constant:

```ts
export const UPDATE_PROGRESS_EVENT = 'ghost://update-progress';
```

Add to the type import block:

```ts
import type {
  ClientInfoDto,
  ContactDto,
  CreatedIdentityDto,
  IdentityStatusDto,
  InboundMessageEvent,
  InviteDto,
  MessageDto,
  UpdateAvailableDto
} from './types';
```

Append at the bottom of the file:

```ts
export async function checkForUpdate(): Promise<UpdateAvailableDto | null> {
  return invoke('check_for_update');
}

export async function downloadAndInstallUpdate(): Promise<void> {
  return invoke('download_and_install_update');
}

export interface UpdateProgress {
  chunk: number;
  total: number;
}

export async function onUpdateProgress(
  cb: (p: UpdateProgress) => void
): Promise<UnlistenFn> {
  return listen<UpdateProgress>(UPDATE_PROGRESS_EVENT, (event) => cb(event.payload));
}
```

### Step 6.3: Verify

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors. The `@tauri-apps/plugin-updater` JS package is technically optional here — we drive everything through the Rust commands plus the event listener. Plan 08 does NOT use the JS package's `check`/`downloadAndInstall` exports directly. We still install the package in `package.json` for two reasons: it's the conventional pairing with the Rust plugin, and Plan 09's settings UI may want its TS types.

### Step 6.4: Commit

- [ ] **Run:**

```bash
git add frontend/package.json frontend/pnpm-lock.yaml frontend/src/lib/tauri.ts
git commit -m "feat(frontend): updater bridge wrappers (check, download_and_install, progress events)"
```

---

## Task 7: `UpdateBanner.svelte` component

**Files:**
- Create: `frontend/src/lib/components/UpdateBanner.svelte`

### Step 7.1: Create the component

- [ ] **Create `frontend/src/lib/components/UpdateBanner.svelte`** with content:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    checkForUpdate,
    downloadAndInstallUpdate,
    onUpdateProgress
  } from '$lib/tauri';
  import type { UpdateAvailableDto, UpdateProgress } from '$lib/tauri';

  let update = $state<UpdateAvailableDto | null>(null);
  let dismissed = $state(false);
  let downloading = $state(false);
  let progress = $state<UpdateProgress | null>(null);
  let errorMsg = $state<string | null>(null);
  let unlistenProgress: (() => void) | null = null;
  let pollHandle: ReturnType<typeof setInterval> | null = null;

  const POLL_MS = 60 * 60 * 1000; // 1 hour

  async function poll() {
    try {
      const result = await checkForUpdate();
      if (result) {
        update = result;
      }
    } catch (e) {
      // Silent for poll failures; logged via console for debugging.
      console.warn('update check failed:', e);
    }
  }

  async function restart() {
    if (!update || downloading) return;
    downloading = true;
    errorMsg = null;
    try {
      await downloadAndInstallUpdate();
      // Process exits via installer; we won't reach here normally.
    } catch (e) {
      errorMsg = String(e);
      downloading = false;
    }
  }

  function dismiss() {
    dismissed = true;
  }

  onMount(() => {
    void poll();
    pollHandle = setInterval(poll, POLL_MS);

    void onUpdateProgress((p) => {
      progress = p;
    }).then((u) => {
      unlistenProgress = u;
    });

    return () => {
      if (pollHandle) clearInterval(pollHandle);
      unlistenProgress?.();
    };
  });

  let visible = $derived(update !== null && !dismissed);
</script>

{#if visible && update}
  <div
    style="position: sticky; top: 0; left: 0; right: 0; background: #3a2e15; color: #f5d486; padding: 0.6rem 1rem; display: flex; align-items: center; gap: 1rem; border-bottom: 1px solid #5a4625; font-size: 0.9rem; z-index: 100;"
    role="status"
    aria-live="polite"
  >
    {#if downloading}
      <span style="flex: 1;">
        Скачивается обновление…
        {#if progress && progress.total > 0}
          {Math.round((progress.chunk / progress.total) * 100)}%
        {/if}
      </span>
    {:else}
      <span style="flex: 1;">↑ Доступна Ghost {update.version}</span>
      <button
        type="button"
        onclick={restart}
        style="padding: 0.3rem 0.8rem; background: #f5d486; color: #1a1c22; border: 0; border-radius: 4px; cursor: pointer; font-weight: 500;"
      >
        Перезапустить
      </button>
      <button
        type="button"
        onclick={dismiss}
        style="padding: 0.3rem 0.8rem; background: transparent; color: inherit; border: 1px solid #5a4625; border-radius: 4px; cursor: pointer;"
      >
        Позже
      </button>
    {/if}

    {#if errorMsg}
      <span style="color: #ff6464; margin-left: 0.5rem;">{errorMsg}</span>
    {/if}
  </div>
{/if}
```

### Step 7.2: Verify

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors.

If `$derived(update !== null && !dismissed)` triggers a TS error about narrowing in the `{#if}` block, an explicit type guard inside helps; the spec already uses `{#if visible && update}` so the inner `update.version` narrows correctly.

### Step 7.3: Commit

- [ ] **Run:**

```bash
git add frontend/src/lib/components/UpdateBanner.svelte
git commit -m "feat(frontend): UpdateBanner component (check, restart, dismiss, progress)"
```

---

## Task 8: Mount `UpdateBanner` in `+layout.svelte`

**Files:**
- Modify: `frontend/src/routes/+layout.svelte`

### Step 8.1: Update the layout

- [ ] **Edit `frontend/src/routes/+layout.svelte`** — replace the current contents with:

```svelte
<script lang="ts">
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';

  let { children } = $props();
</script>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: #0e0f12;
    color: #e8e8ec;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }
  :global(*) {
    box-sizing: border-box;
  }
  main {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
  }
  .layout-content {
    flex: 1;
    overflow: auto;
  }
</style>

<main>
  <UpdateBanner />
  <div class="layout-content">
    {@render children()}
  </div>
</main>
```

The wrapper structure (`<main>` flex column with the banner sticky at top and the content scrolling below) keeps the chat view's `height: 100%` working correctly (the chat view's flex container fills `.layout-content`).

### Step 8.2: Verify

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors.

### Step 8.3: Commit

- [ ] **Run:**

```bash
git add frontend/src/routes/+layout.svelte
git commit -m "feat(frontend): mount UpdateBanner in root layout"
```

---

## Task 9: Reproducible build flags (`.cargo/config.toml`)

**Files:**
- Create: `.cargo/config.toml`

### Step 9.1: Create the file

- [ ] **Create file `.cargo/config.toml`** at the repo root with content:

```toml
# Reproducible build flags for release artifacts.
#
# --remap-path-prefix: removes absolute build paths from debug info, so
# `cargo build` on different machines (different home directories) produces
# byte-identical binaries.
#
# -C debuginfo=0: drops debug symbols from release builds. Already implied
# by the [profile.release] strip = "symbols" but explicit here so dev
# builds also stay slim.
#
# These flags apply to ALL targets and ALL profiles; the release profile in
# Cargo.toml at the workspace root takes precedence for release-specific
# overrides like LTO and codegen-units.

[build]
rustflags = ["--remap-path-prefix", "${CARGO_MANIFEST_DIR}=."]
```

We do NOT add `-C debuginfo=0` here because the workspace root `Cargo.toml`'s `[profile.release]` already sets `strip = "symbols"`. Adding `debuginfo=0` here would make dev builds also drop debuginfo, hurting `cargo run` debugging.

### Step 9.2: Verify dev build still works

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc check -p ghost-desktop
```

- [ ] **Expected:** `Finished` with no warnings about the new flag. `--remap-path-prefix` is a no-op for incremental builds without paths in the output, but should not break anything.

### Step 9.3: Commit

- [ ] **Run:**

```bash
git add .cargo/config.toml
git commit -m "feat(build): reproducible build flag (--remap-path-prefix)"
```

---

## Task 10: CI workflow (`.github/workflows/ci.yml`)

**Files:**
- Create: `.github/workflows/ci.yml`

### Step 10.1: Create the workflow

- [ ] **Create file `.github/workflows/ci.yml`** with content:

```yaml
name: CI

on:
  push:
    branches: [master]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  rust:
    name: Rust check (Windows)
    runs-on: windows-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust 1.87 (msvc)
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.87
          targets: x86_64-pc-windows-msvc
          components: rustfmt, clippy

      - name: Install Strawberry Perl
        shell: pwsh
        run: choco install strawberryperl -y --no-progress

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Format check
        run: cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check

      - name: Clippy (ghost-app + ghost-desktop)
        run: cargo +1.87-x86_64-pc-windows-msvc clippy -p ghost-app -p ghost-desktop --all-targets -- -D warnings

      - name: Test workspace
        run: cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1

  frontend:
    name: Frontend check (Linux)
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node 20
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Setup pnpm
        uses: pnpm/action-setup@v3
        with:
          version: 10

      - name: Cache pnpm
        uses: actions/cache@v4
        with:
          path: ~/.pnpm-store
          key: ${{ runner.os }}-pnpm-${{ hashFiles('frontend/pnpm-lock.yaml') }}

      - name: Install deps
        run: pnpm --dir frontend install --frozen-lockfile

      - name: Type check
        run: pnpm --dir frontend check

      - name: Build
        run: pnpm --dir frontend build
```

The Rust job runs on `windows-latest` because:
- The project includes Strawberry Perl as a hard dep for SQLCipher.
- We commit to the MSVC toolchain (`+1.87-x86_64-pc-windows-msvc`).
- Cross-compiling to Windows from Linux is theoretically possible but adds complexity.

The frontend job runs on `ubuntu-latest` because Vite + svelte-check are platform-independent and Ubuntu runners are faster + cheaper.

### Step 10.2: Verify YAML syntax

- [ ] **Run:** `cat .github/workflows/ci.yml` and visually check indentation. (No local linter required; GitHub will validate when the workflow runs.)

### Step 10.3: Commit

- [ ] **Run:**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): GitHub Actions CI workflow (Rust check + frontend check)"
```

The workflow will only fire when pushed to GitHub (which the user does separately). Locally it sits inert.

---

## Task 11: Release workflow (`.github/workflows/release.yml`)

**Files:**
- Create: `.github/workflows/release.yml`

### Step 11.1: Create the workflow

- [ ] **Create file `.github/workflows/release.yml`** with content:

```yaml
name: Release

on:
  push:
    tags: ['v*']

env:
  CARGO_TERM_COLOR: always

jobs:
  release-windows:
    name: Build + sign + release (Windows)
    runs-on: windows-latest
    permissions:
      contents: write   # required for gh release create

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Guard placeholder owner/repo
        shell: pwsh
        run: |
          $conf = Get-Content apps/ghost-desktop/tauri.conf.json -Raw
          if ($conf -match 'OWNER_REPLACE_ME' -or $conf -match 'REPO_REPLACE_ME') {
            Write-Error "tauri.conf.json still contains OWNER_REPLACE_ME or REPO_REPLACE_ME. Fill in the real github coordinates before tagging a release. See docs/release-process.md."
            exit 1
          }
          if ($conf -match 'RWQ_PASTE_THE_BASE64_LINE_FROM') {
            Write-Error "tauri.conf.json still contains the placeholder pubkey. Run scripts/generate-minisign-keypair.sh and paste the real pubkey before releasing."
            exit 1
          }

      - name: Install Rust 1.87 (msvc)
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.87
          targets: x86_64-pc-windows-msvc

      - name: Install Strawberry Perl
        shell: pwsh
        run: choco install strawberryperl -y --no-progress

      - name: Install minisign
        shell: pwsh
        run: choco install minisign -y --no-progress

      - name: Setup Node 20
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Setup pnpm
        uses: pnpm/action-setup@v3
        with:
          version: 10

      - name: Install tauri-cli
        shell: pwsh
        run: cargo install tauri-cli --version "^2" --locked

      - name: Compute SOURCE_DATE_EPOCH
        id: source-date
        shell: bash
        run: echo "epoch=$(git log -1 --format=%ct)" >> $GITHUB_OUTPUT

      - name: Frontend deps + build
        shell: bash
        run: pnpm --dir frontend install --frozen-lockfile

      - name: Tauri build
        shell: bash
        env:
          SOURCE_DATE_EPOCH: ${{ steps.source-date.outputs.epoch }}
        run: cargo +1.87-x86_64-pc-windows-msvc tauri build --config apps/ghost-desktop/tauri.conf.json

      - name: Locate MSI
        id: artifacts
        shell: bash
        run: |
          MSI="$(ls target/release/bundle/msi/ghost-desktop_*.msi | head -1)"
          if [[ -z "${MSI}" ]]; then
            echo "ERROR: no .msi produced under target/release/bundle/msi/" >&2
            exit 1
          fi
          echo "msi_path=${MSI}" >> $GITHUB_OUTPUT
          echo "msi_name=$(basename ${MSI})" >> $GITHUB_OUTPUT

      - name: Sign MSI with minisign
        shell: bash
        env:
          MINISIGN_PRIVATE_KEY: ${{ secrets.MINISIGN_PRIVATE_KEY }}
        run: |
          # Write the secret to a temp file (minisign needs the key as a file).
          # Use a process-scoped tmpdir; GH Actions cleans it on job exit.
          KEYFILE="${RUNNER_TEMP}/minisign.key"
          printf '%s\n' "${MINISIGN_PRIVATE_KEY}" > "${KEYFILE}"
          chmod 600 "${KEYFILE}"

          # Sign the MSI.
          # -W tells minisign to read passphrase from MINISIGN_PASSWORD env var.
          # We set MINISIGN_PASSWORD to empty since the CI key was generated
          # with an empty passphrase (see release-process.md). If you used
          # a non-empty passphrase locally, set the MINISIGN_PASSWORD secret
          # in repo settings as well.
          MINISIGN_PASSWORD="${{ secrets.MINISIGN_PASSWORD }}" \
          minisign -S -W -s "${KEYFILE}" -m "${{ steps.artifacts.outputs.msi_path }}"

          rm -f "${KEYFILE}"

      - name: Generate latest.json manifest
        shell: bash
        run: |
          VERSION="${GITHUB_REF_NAME#v}"   # strip leading v
          MSI_NAME="${{ steps.artifacts.outputs.msi_name }}"
          MSI_URL="https://github.com/${GITHUB_REPOSITORY}/releases/download/${GITHUB_REF_NAME}/${MSI_NAME}"
          SIG=$(cat "${{ steps.artifacts.outputs.msi_path }}.sig")
          PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

          # Notes from the tag annotation (if any).
          NOTES=$(git tag -l --format='%(contents)' "${GITHUB_REF_NAME}" | head -c 4000)

          jq -n \
            --arg version "${VERSION}" \
            --arg notes "${NOTES}" \
            --arg pub_date "${PUB_DATE}" \
            --arg url "${MSI_URL}" \
            --arg signature "${SIG}" \
            '{
              version: $version,
              notes: $notes,
              pub_date: $pub_date,
              platforms: {
                "windows-x86_64": {
                  signature: $signature,
                  url: $url
                }
              }
            }' > latest.json

          cat latest.json

      - name: Create GitHub Release
        shell: bash
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          NOTES_FILE="${RUNNER_TEMP}/release-notes.md"
          git tag -l --format='%(contents)' "${GITHUB_REF_NAME}" > "${NOTES_FILE}"
          if [[ ! -s "${NOTES_FILE}" ]]; then
            echo "Release ${GITHUB_REF_NAME}" > "${NOTES_FILE}"
          fi

          gh release create "${GITHUB_REF_NAME}" \
            --title "Ghost ${GITHUB_REF_NAME}" \
            --notes-file "${NOTES_FILE}" \
            "${{ steps.artifacts.outputs.msi_path }}" \
            "${{ steps.artifacts.outputs.msi_path }}.sig" \
            latest.json
```

The placeholder guard (first step) is a hard gate: if the user forgot to fill in the actual GitHub coordinates or paste the real pubkey, the release fails BEFORE any signing happens. Cheap insurance.

`MINISIGN_PASSWORD` is referenced but not required if you generated a passphrase-less key. If the implementer's key has a passphrase, add a second repo secret `MINISIGN_PASSWORD` containing it. Document this in Task 12's runbook.

### Step 11.2: Commit

- [ ] **Run:**

```bash
git add .github/workflows/release.yml
git commit -m "feat(ci): release workflow (build + sign + GitHub release on v* tag)"
```

---

## Task 12: `docs/release-process.md` runbook

**Files:**
- Create: `docs/release-process.md`

### Step 12.1: Create the runbook

- [ ] **Create file `docs/release-process.md`** with content:

````markdown
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
````

### Step 12.2: Commit

- [ ] **Run:**

```bash
git add docs/release-process.md
git commit -m "docs: add release process runbook for Plan 08"
```

---

## Task 13: End-to-end smoke + tag `plan-08-complete`

The full e2e smoke test (cutting two adjacent releases on a real GitHub repo
and watching one update the other) requires a GitHub repo to be set up with
the secrets configured. The implementer in subagent-driven mode CANNOT do
this — it requires interactive GitHub UI access. Document the smoke as a
manual checklist that the user runs at their convenience.

**Files:**
- Create: `scripts/smoke-test-plan-08.md`

### Step 13.1: Create the smoke checklist

- [ ] **Create file `scripts/smoke-test-plan-08.md`** with content:

````markdown
# Plan 08 smoke test — auto-update via GitHub Releases

Two-stage manual test. Requires:
- A GitHub repo with the Ghost code pushed.
- The MINISIGN_PRIVATE_KEY secret configured (per docs/release-process.md).
- The `OWNER_REPLACE_ME/REPO_REPLACE_ME` placeholders substituted in
  `tauri.conf.json`.
- A Windows machine to install / observe the update.

Expected runtime: ~30-60 minutes including waiting for two CI builds.

## Stage 1: Cut and install v0.0.1

### Step 1.1: Bump to v0.0.1 (or your initial version)

Edit `apps/ghost-desktop/tauri.conf.json`'s `version` to `0.0.1`. Commit:

```bash
git commit -am "chore: v0.0.1"
git tag -a v0.0.1 -m "First release"
git push origin master --tags
```

### Step 1.2: Wait for CI

Watch the Release workflow on GitHub Actions. Should take ~10 minutes cold,
~3 minutes warm.

### Step 1.3: Verify the release

Visit `https://github.com/<owner>/<repo>/releases/tag/v0.0.1`. Confirm:
- `Ghost_0.0.1_x64_en-US.msi` (or similar; exact name set by Tauri)
- `Ghost_0.0.1_x64_en-US.msi.sig`
- `latest.json` (version: "0.0.1")

### Step 1.4: Install on a test Windows machine

Download `Ghost_0.0.1_x64_en-US.msi` and run it. SmartScreen will show
"Windows protected your PC" — click "More info" → "Run anyway" (this goes
away in Plan 09 with EV cert).

After install, launch Ghost. Confirm:
- Onboarding screen appears.
- Create an identity.
- Note the Ghost ID for verification.

Leave Ghost running.

## Stage 2: Cut v0.0.2 and confirm auto-update

### Step 2.1: Bump to v0.0.2

```bash
# In the repo:
# Edit apps/ghost-desktop/tauri.conf.json: "version": "0.0.2"
git commit -am "chore: v0.0.2"
git tag -a v0.0.2 -m "Test release for auto-update flow"
git push origin master --tags
```

### Step 2.2: Wait for the v0.0.2 release

Watch GitHub Actions. ~3 minutes warm. Confirm the v0.0.2 Release has the
three expected assets.

### Step 2.3: Observe the update banner on the v0.0.1 instance

Within ~1 minute (auto-poll on app start; if the app has been open longer
than the 1-hour interval, restart it to trigger a fresh poll), the yellow
banner should appear at the top:

```
↑ Доступна Ghost 0.0.2     [Перезапустить] [Позже]
```

### Step 2.4: Click "Перезапустить"

The banner switches to the progress state:

```
↓ Скачивается обновление…  53%
```

When the download completes, the Tauri/Wix MSI installer takes over.
The current Ghost process exits.

### Step 2.5: Verify v0.0.2 is running

The new Ghost.exe should auto-launch after the installer. Open it.

Confirm:
- The same identity from Stage 1 is loaded (same Ghost ID).
- All previous contacts and messages are intact.
- Open About / window title indicates v0.0.2 (if About screen exists; else
  inspect Help → About in Windows Programs and Features).

## PASS criteria

All steps complete. v0.0.1 instance found v0.0.2, downloaded, verified,
installed, and re-launched with intact user state.

## Failure modes and recovery

| Symptom | Likely cause | Recovery |
|---|---|---|
| CI fails on placeholder guard | OWNER_REPLACE_ME/etc still in tauri.conf.json | Substitute, re-tag (skip v0.0.X to a fresh number) |
| CI fails on minisign step | MINISIGN_PRIVATE_KEY secret missing or wrong format | Verify in repo Settings → Secrets |
| Banner never appears | Wrong endpoint URL in tauri.conf.json | Check it points at `releases/latest/download/latest.json` |
| Banner appears but signature verification fails (silent abort) | Wrong pubkey embedded vs key used by CI | Regenerate key, re-bump versions |
| MSI installer hangs after "Перезапустить" | Tauri-WiX installer corrupt | Bug in build pipeline; investigate `cargo tauri build` output |

## Cleanup

After the smoke passes:
- Optionally delete the test releases via `gh release delete v0.0.1 v0.0.2`.
- Or leave them as the actual MVP-1 first releases.
````

### Step 13.2: Final workspace verification

Same fmt + clippy + tests pass:

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
cargo +1.87-x86_64-pc-windows-msvc clippy -p ghost-app -p ghost-desktop --all-targets -- -D warnings
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
pnpm --dir frontend check
```

- [ ] **Expected:**
  - fmt clean
  - clippy clean on the two new crates
  - all 176 prior tests still pass (Plan 08 doesn't add new Rust tests)
  - svelte-check 0 errors

If clippy fails on `crates/ghost-app/src/commands/updater.rs`, read the warning:
- Unused-import on `Emitter` if you didn't actually emit — should be used in `download_and_install_update`.
- `unused_must_use` on `let _ = app_clone.emit(...)` — fine to ignore by design.

### Step 13.3: Commit

- [ ] **Run:**

```bash
git add scripts/smoke-test-plan-08.md
git commit -m "test(plan-08): manual smoke checklist for two-release auto-update flow"
```

### Step 13.4: Tag

- [ ] **Run:**

```bash
git tag -a plan-08-complete -m "Plan 08 — Updater + Release Pipeline

Working end-to-end auto-update for Windows desktop binaries via GitHub
Releases. Manifest signed with offline minisign key. Old version finds +
downloads + verifies + installs new version.

Crates added: none (ghost-app extended with updater commands; ghost-desktop
wires the plugin). Frontend: UpdateBanner.svelte with reactive progress.

CI:
- .github/workflows/ci.yml — fmt + clippy + test + frontend check on push/PR.
- .github/workflows/release.yml — tag-triggered build + sign + GH release.

Reproducible build flags applied via .cargo/config.toml + SOURCE_DATE_EPOCH.

Real Windows multi-size icon replaces 1x1 placeholder.

Deferred to Plan 09:
- Win EV code-signing cert
- macOS Apple Dev account + notarization + .dmg
- Linux builds
- N-of-M signing
- Sigstore Rekor
- 100% reproducible builds (docker)
- Custom domain / Cloudflare update channel
- Settings UI
- Inline changelog
- min_supported field handling
- Update kill-switch / revocation"
```

Verify:

- [ ] **Run:** `git tag -l plan-08-complete`
- [ ] **Expected:** `plan-08-complete` listed.

---

## Self-review (mental — implementer should re-check before claiming Plan 08 done)

1. **Spec coverage:**
   - tauri-plugin-updater integration → Tasks 3, 5. ✓
   - Manifest schema + signing → Task 11 (release.yml). ✓
   - Pubkey embedded → Task 3 (tauri.conf.json). ✓
   - Privkey as GH secret → Task 12 (release-process.md instructs upload). ✓
   - Real icons → Task 2. ✓
   - UpdateBanner with toast UX → Tasks 7, 8. ✓
   - Reproducible build flags → Task 9. ✓
   - CI matrix Windows-only → Tasks 10, 11. ✓
   - GitHub Releases as update channel → Task 11. ✓
   - Polling cadence (start + 1h) → Task 7 (UpdateBanner). ✓
   - End-to-end smoke → Task 13. ✓
   - Out-of-scope items called out in Task 13's tag message. ✓

2. **Placeholder scan:** No "TODO" or "TBD" in actionable steps. The two literal placeholders (`OWNER_REPLACE_ME/REPO_REPLACE_ME` in `tauri.conf.json` and `RWQ_PASTE_THE_BASE64_LINE_FROM...`) are intentional config-time substitutions, guarded by Task 11's CI step that fails the build if they leak through to a release.

3. **Type consistency:**
   - `UpdateAvailableDto` shape matches between `dto.rs` and `types.ts` (version: string, notes: Option/null, release_date: Option/null). ✓
   - `check_for_update` returns `Option<UpdateAvailableDto>` in Rust, `UpdateAvailableDto | null` in TS. ✓
   - `UPDATE_PROGRESS_EVENT = 'ghost://update-progress'` matches the emit string in `updater.rs`. ✓
   - `UpdateProgress { chunk, total }` matches `ProgressPayload { chunk, total }` in Rust. ✓

---

**End of Plan 08.** After this plan completes, MVP-1 is shippable. The next plan (09) will productionize signing infrastructure and add cross-platform builds.
