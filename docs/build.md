# Build Guide

This project uses a Python backend, a React frontend, and a Tauri shell.

## Common prerequisites
- Node.js 18+ (or 20+)
- Rust (stable toolchain)
- Python 3.10+

## Common setup
```bash
python -m venv .venv
.\.venv\Scripts\activate  # Windows
source .venv/bin/activate # Linux/macOS
pip install -r requirements.txt
npm install
npm --prefix frontend install
```

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
