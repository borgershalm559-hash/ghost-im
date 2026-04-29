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
