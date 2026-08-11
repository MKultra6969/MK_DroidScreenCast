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

## External tools (adb / scrcpy)

The app downloads `adb` and `scrcpy` into `downloads/` on first run. Both are
pinned to a known revision and verified by SHA-256 **before** extraction — a
truncated, corrupted or swapped archive is deleted and the run fails loudly
instead of installing an unchecked binary.

| Tool | Pinned in | Integrity source |
|---|---|---|
| scrcpy | `SCRCPY_PINNED_VERSION` in `mkdsc/tools.py` | `SHA256SUMS.txt` published in the GitHub release |
| platform-tools (adb) | `PLATFORM_TOOLS_REVISION` in `mkdsc/tools.py` | `PLATFORM_TOOLS_SHA256`, hard-coded in the same file |

### Why platform-tools checksums are hard-coded

Google publishes no checksum next to `platform-tools-latest-*.zip`. Checksums
*are* published in
[`repository2-3.xml`](https://dl.google.com/android/repository/repository2-3.xml),
but only as **SHA-1**, and only for versioned URLs
(`platform-tools_r37.0.1-linux.zip`), never for the `latest` alias. Fetching that
manifest at runtime would also mean trusting a second network response to
validate the first, which buys little.

So the app downloads the **versioned** archive and checks a SHA-256 recorded in
the repo. Versioned archives on `dl.google.com` are kept indefinitely (r33 still
resolves today), so pinning does not rot. The values in `PLATFORM_TOOLS_SHA256`
were computed from the downloaded files, and their SHA-1 cross-checked against
the manifest.

This protects against a corrupted download or a tampered mirror. It is not a
defence against a compromised Google signing chain — for that you would need a
GPG trust path Google does not offer here.

### Bumping the pinned versions

**scrcpy** — set `SCRCPY_PINNED_VERSION`, then re-check that the flags the app
passes still exist. The full flag list lives in the comment above
`SCRCPY_VERIFIED_MIN`; compare against `app/src/cli.c` for the new tag. Update
`SCRCPY_VERIFIED_MIN` / `SCRCPY_VERIFIED_MAX` once verified. Checksums need no
manual work — they are read from the release's `SHA256SUMS.txt`.

**platform-tools** — pick the revision from `repository2-3.xml`, then:

```bash
REV=37.0.1
for os in linux win darwin; do
  url="https://dl.google.com/android/repository/platform-tools_r${REV}-${os}.zip"
  curl -sLO "$url"
  echo "$os $(sha256sum "platform-tools_r${REV}-${os}.zip")"
done
```

Cross-check each file's `sha1sum` against the `<checksum type="sha1">` entry in
the manifest before committing the new SHA-256 values.

### Overrides

| Variable | Effect |
|---|---|
| `MKDSC_ADB_PATH` | use this adb, skip the download entirely |
| `MKDSC_SCRCPY_PATH` | use this scrcpy, skip the download entirely |
| `MKDSC_SCRCPY_VERSION` | download a different scrcpy release (e.g. `3.3.4`) |

A scrcpy outside the tested range still starts, but logs a warning and shows it
in the UI — the flag set is only verified for the range in `mkdsc/tools.py`.

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
