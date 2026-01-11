<div align="center">

# MK DroidScreenCast v0.1.0

**Universal Android device manager built on ADB and Scrcpy**
<br>
*Control your phone from a PC over USB or Wi-Fi using the CLI or the Web panel.*

[![Python](https://img.shields.io/badge/Python-3.8%2B-blue?style=for-the-badge&logo=python&logoColor=white)](https://www.python.org/)
[![FastAPI](https://img.shields.io/badge/FastAPI-Web_Panel-009688?style=for-the-badge&logo=fastapi&logoColor=white)](https://fastapi.tiangolo.com/)
[![Scrcpy](https://img.shields.io/badge/Powered_by-Scrcpy-green?style=for-the-badge&logo=android)](https://github.com/Genymobile/scrcpy)
[![Version](https://img.shields.io/badge/Version-0.1.0-2ea44f?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-WTFPL-red?style=for-the-badge)](http://www.wtfpl.net/)

</div>

---

## About

MK DroidScreenCast is a wrapper around `adb` and `scrcpy` that simplifies connecting and mirroring Android devices on a PC. The app prepares its runtime, downloads the required tools, and provides both CLI and Web interfaces.

Modes:
1.  CLI: interactive terminal flow with connection and scrcpy settings.
2.  Web Panel: browser UI with live device status and presets.

---

## Interface

> <img width="3838" height="1840" alt="screenshot" src="https://github.com/user-attachments/assets/e83c7060-2964-4dd9-b4f1-c3e1e471a38b" />

---

## Features

### Setup
*   Auto bootstrap: `gateway.py` creates `.venv` and installs dependencies on first run.
*   Auto download: grabs platform-tools (ADB) and Scrcpy for your OS.

### Connections
*   USB connection.
*   Wireless pairing (Android 11+).
*   USB -> Wi-Fi (TCP/IP mode).
*   Saved devices list for quick reconnect.

### Scrcpy
*   Quality presets: 1080p, 2K, 4K, or custom.
*   Bitrate and max size controls.
*   Keyboard modes: `UHID`, `SDK`, `AOA`.
*   Options: keep screen awake, show touches, turn screen off, fullscreen, no audio.

### Web Panel
*   Live updates via WebSocket.
*   Device and preset management.
*   Start scrcpy from the browser.
*   Diagnostics: download `logs.zip` and check updates.
*   Language switcher: EN/RU.

---

## Installation

### Requirements
*   Python 3.8 or newer.
*   Windows, Linux, or macOS.

### Quick start

1.  **Clone the repo:**
    ```bash
    git clone https://github.com/MKultra6969/MK_DroidScreenCast
    cd MK_DroidScreenCast
    ```

2.  **Run the app:**
    ```bash
    python gateway.py
    ```

On first run it will create `.venv`, install dependencies, and download ADB/Scrcpy.

### Manual setup (optional)

```bash
python -m venv .venv
.\.venv\Scripts\activate
pip install -r requirements.txt
```

---

## Usage

Main menu:

```bash
python gateway.py
```

Choose a mode:

### 1. CLI Mode
Follow the prompts for connection and scrcpy settings.

### 2. Web Panel
*   Open: `http://localhost:6969`
*   Manage devices and launch scrcpy from the browser.

Alternative entry points:
```bash
python screencast.py
python web_panel.py
```

---

## Configuration

Settings live in `config.json` (language, presets, and web panel settings). Edit it while the app is closed.

---

## Project structure

```text
MK_droidScreenCast/
├── mkdsc/              # Core logic (CLI, web, tools)
├── templates/          # HTML templates
├── static/             # CSS, JS and web assets
├── downloads/          # ADB/Scrcpy and updates
├── logs/               # Logs and diagnostics
├── config.json         # Configuration and saved devices
├── gateway.py          # Bootstrap launcher
├── screencast.py       # CLI-only entry point
├── web_panel.py        # Web-only entry point
└── requirements.txt    # Python dependencies
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
