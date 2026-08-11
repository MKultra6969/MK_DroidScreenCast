# Build Guide

This project uses a Python backend, a React frontend, and a Tauri shell.

## Common prerequisites
- Node.js 18+ (or 20+)
- Rust **1.85 or newer** — `src-tauri` uses edition 2024, and older toolchains
  fail with a confusing "unknown edition" error
- Python 3.10+

## Common setup
```bash
python -m venv .venv
.\.venv\Scripts\activate  # Windows
source .venv/bin/activate # Linux/macOS
pip install -r requirements.txt   # requirements-dev.txt to also get PyInstaller + pytest
npm install
npm --prefix frontend install
```

## Checks
```bash
python -m pytest                  # backend tests
npm --prefix frontend run lint
npm --prefix frontend run typecheck
npm --prefix frontend run build   # also refreshes the committed static/ bundle
```

`static/` holds the bundle served by the web panel (`python web_panel.py`).
Rebuild and commit it whenever `frontend/src` changes, or the web UI will lag
behind the API.

## Windows (release build)
```bash
npm run tauri:backend:build
npm run tauri build
```

## Linux (release build)
Install Tauri system dependencies for your distro (WebKit2GTK, GTK, and system tray libs).
```bash
npm run tauri:backend:build
npm run tauri build
```

## macOS (release build)
Install Xcode Command Line Tools: `xcode-select --install`.
```bash
npm run tauri:backend:build
npm run tauri build
```

## One-shot build
```bash
npm run tauri:build:full
```

## Updater signing key (required for release builds)

The desktop app updates itself through `tauri-plugin-updater`, which only
installs **signed** artifacts. `bundle.createUpdaterArtifacts` is enabled, so a
release build fails until a signing key exists. Generate one once:

```bash
npm run tauri signer generate -- -w ~/.tauri/mkdsc.key
```

Then:

1. Put the **public** key into `src-tauri/tauri.conf.json` →
   `plugins.updater.pubkey` (it is currently an empty placeholder).
2. Keep the **private** key secret. Losing it means existing installs can never
   be updated again.
3. Export it before building a release — `.env` files are not read here:

   ```bash
   export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/mkdsc.key)"
   export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
   ```

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content ~/.tauri/mkdsc.key -Raw
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
   ```

4. For CI, store both as the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; `.github/workflows/release.yml` picks
   them up and publishes `latest.json` alongside the installers.

Until the public key is filled in, the app still works — "Check updates" just
falls back to opening the GitHub release page instead of updating in place.

## How updates reach users

- **Desktop app** — `tauri-plugin-updater` reads
  `https://github.com/MKultra6969/MK_DroidScreenCast/releases/latest/download/latest.json`,
  verifies the signature, installs and relaunches.
- **Web panel / CLI from a clone** — no self-update. `GET /api/update/check`
  compares versions and the UI offers the release page. The old behaviour
  (downloading a source zipball over the install directory) is gone: it needed
  admin rights, shipped `.py` files an installed build never runs, and could
  not restart anything.
