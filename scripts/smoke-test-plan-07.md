# Plan 07 smoke test — two instances chat via the GUI

This is the canonical regression check after any change to Plan 07's Tauri /
frontend layer. Expected runtime: ~5 minutes if everything works.

## Setup

Two separate terminal windows.

**Terminal A (Alice profile):**
```bash
./scripts/launch-ghost-desktop.sh alice
```

**Terminal B (Bob profile):**
```bash
./scripts/launch-ghost-desktop.sh bob
```

Each opens a Ghost window. Wait for both to render the onboarding screen (5–15 s
on a warm cache; up to a minute cold).

## Steps

### 1. Alice — onboarding

- In Alice's window: enter display name `Alice` (passphrase blank), click "Create identity".
- Expected: redirects to home screen showing Alice's Ghost ID + 4×4 fingerprint.

### 2. Bob — onboarding

- In Bob's window: enter display name `Bob`, click "Create identity".
- Expected: home screen showing Bob's Ghost ID + fingerprint. **Different from Alice's**.

### 3. Alice — generate invite

- In Alice's window, click "Generate invite".
- Expected: a `ghostinvite1q...` string appears in the textarea.
- Click "Copy".

### 4. Bob — add Alice as contact

- In Bob's window, paste the invite string into the "Add contact" textarea.
- Click "Add contact".
- Expected: green "Contact added." message; Alice's fingerprint appears in the contacts list.

### 5. Alice — observe new contact

- In Alice's window, the contacts list should show Bob's fingerprint within ~2 seconds (the inbox bridge fires after the Welcome envelope is processed).
  - If Alice's contacts list is still empty after 5 seconds: refresh the home page (browsers reload via F5; Tauri windows can be closed and reopened by re-running the launch script — `GHOST_HOME` persists).

### 6. Bob → Alice message

- In Bob's window, click on Alice's contact entry → chat view opens.
- Type `hello alice` → click "Send".
- Expected: message appears immediately on Bob's right side (outgoing-blue).

### 7. Alice receives

- In Alice's window, click on Bob's contact entry → chat view opens.
- Expected: `hello alice` appears on the left (incoming-grey) within ~1 second of Bob sending.
  - If not present, wait 5 seconds (the inbox watcher polls every 250 ms; first message has cold-cache cost).

### 8. Alice → Bob message

- In Alice's chat view with Bob, type `hi bob` → click "Send".
- Expected: outgoing message on Alice's right.

### 9. Bob receives

- In Bob's chat view with Alice, expected: `hi bob` appears on the left.

## PASS criteria

All 9 steps complete. Both windows show the symmetric conversation:
- Alice's chat with Bob: `hello alice` (left), `hi bob` (right).
- Bob's chat with Alice: `hello alice` (right), `hi bob` (left).

## Cleanup

```bash
rm -rf .tmp/profiles
```

(Or close both windows; `.tmp/profiles` persists for re-runs.)

## Common issues

- **Both windows show Alice's data after first launch** → kept the same `GHOST_HOME`. Pass distinct profile names to the launch script.
- **"Failed to load: client open" repeatedly** → port collision on libp2p loopback. Both clients listen on `/ip4/127.0.0.1/udp/0/quic-v1` (port 0 = OS-assigned), so collisions are theoretically impossible — if it happens, close both windows, `pkill ghost-desktop`, retry.
- **`hello alice` never appears in Alice's window** → the inbox-bridge watcher task is stuck. Check Alice's terminal for `inbox process error` or `inbox watcher scan failed` messages. Re-running both windows usually clears it.
- **Tauri window blank / white** → the frontend bundle was not built. Run `pnpm --dir frontend build` then re-launch.
