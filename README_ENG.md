[🇷🇺Русский](https://github.com/MKultra6969/MK_DroidScreenCast/blob/main/README.md)
<div align="center">

# 📱 MK DroidScreenCast v2.0.0

**Desktop Android control center built on ADB and Scrcpy**
<br>
*Connect, mirror, record, and manage devices from one native app.*

[![Tauri](https://img.shields.io/badge/Tauri-Desktop-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Scrcpy](https://img.shields.io/badge/Powered_by-Scrcpy-green?style=for-the-badge&logo=android)](https://github.com/Genymobile/scrcpy)
[![Version](https://img.shields.io/badge/Version-2.0.0-2ea44f?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-WTFPL-red?style=for-the-badge)](http://www.wtfpl.net/)

</div>

---

## About

MK DroidScreenCast is a full desktop application that wraps `adb` and `scrcpy` with a focused UI. All of the logic lives inside the app itself — no helper process, no local port — it downloads the tools it needs and keeps everything inside a single window.

---

## Interface

> <img width="2490" height="1312" alt="showcase" src="https://github.com/user-attachments/assets/2d5e608a-a59e-494e-bb03-e8499dd1f990" />

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
See `docs/build.md` for prerequisites (Node 18+, Rust 1.85+) and build commands.

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
├── frontend/          # Desktop UI (React + Tauri)
├── src-tauri/         # Rust: app logic and IPC
├── downloads/         # ADB/Scrcpy cache
├── logs/              # Logs and diagnostics
├── config.json        # App settings
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
