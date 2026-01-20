[🇷🇺Русский](https://github.com/MKultra6969/MK_DroidScreenCast/blob/main/README.md)
<div align="center">

# MK DroidScreenCast v1.0.0

**Desktop Android control center built on ADB and Scrcpy**
<br>
*Connect, mirror, record, and manage devices from one native app.*

[![Python](https://img.shields.io/badge/Python-3.10%2B-blue?style=for-the-badge&logo=python&logoColor=white)](https://www.python.org/)
[![Tauri](https://img.shields.io/badge/Tauri-Desktop-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Scrcpy](https://img.shields.io/badge/Powered_by-Scrcpy-green?style=for-the-badge&logo=android)](https://github.com/Genymobile/scrcpy)
[![Version](https://img.shields.io/badge/Version-1.0.0-2ea44f?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-WTFPL-red?style=for-the-badge)](http://www.wtfpl.net/)

</div>

---

## About

MK DroidScreenCast is a full desktop application that wraps `adb` and `scrcpy` with a focused UI. It runs a local backend, handles tool downloads, and keeps everything inside a single app window.

---

## Interface

> <img width="3840" height="2100" alt="showcase" src="https://github.com/user-attachments/assets/bf3c59fb-137c-48ce-8a27-f6a4b6339407" />

---

## Features

### Devices
*   USB, Wi-Fi pairing (Android 11+), USB to Wi-Fi (TCP/IP).
*   Saved devices and quick connect.
*   Auto connection preference.

### Scrcpy + Recording
*   Presets for bitrate and max size.
*   Keyboard modes and common toggles (stay awake, show touches, fullscreen, no audio, turn screen off).
*   Recording HUD with format, audio source, and output folder.

### Files + Diagnostics
*   File manager (push/pull) and screenshot gallery.
*   Export `logs.zip` and check for updates.
*   Config editor and RU/EN UI.

---

## Installation

### Release build
1.  Download the latest installer from GitHub Releases: https://github.com/MKultra6969/MK_DroidScreenCast/releases
2.  Install and launch MK DroidScreenCast.

### Build from source
See `docs/build.md` for prerequisites (Node 18+, Rust, Python 3.10+) and build commands.

---

## Usage

1.  Open MK DroidScreenCast.
2.  Connect your device via USB or Wi-Fi pairing.
3.  Pick a scrcpy preset and start mirroring.
4.  Use Recording, Files, and Diagnostics as needed.

---

## Configuration

Settings live in `config.json` and can be edited in-app under Settings > Config.

---

## Project structure

```text
MK_DroidScreenCast/
├── frontend/          # Desktop UI (Tauri)
├── src-tauri/         # Rust shell
├── mkdsc/             # Python backend (ADB/Scrcpy logic)
├── bin/               # Bundled backend binaries
├── downloads/         # ADB/Scrcpy cache
├── logs/              # Logs and diagnostics
├── config.json        # App settings
├── tauri_backend.py   # Backend entry point
└── docs/              # Build notes
```

---

## Phone preparation

1.  Settings -> About phone -> tap Build number 7 times.
2.  Settings -> System -> Developer options.
3.  Enable USB debugging.
4.  For Wi-Fi (Android 11+), enable Wireless debugging.

---

## Author

**MKultra69**

*   GitHub: [@MKultra6969](https://github.com/MKultra6969)
*   Telegram Channel: [@MKplusULTRA](https://t.me/MKplusULTRA)

## P.S.
* Everything is obvious, license as always, attitude to people as always.
