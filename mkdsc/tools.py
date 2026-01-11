import os
import platform
import shutil
import subprocess
from pathlib import Path

import requests

from .paths import DOWNLOADS_DIR

PLATFORM_TOOLS_URLS = {
    "windows": "https://dl.google.com/android/repository/platform-tools-latest-windows.zip",
    "linux": "https://dl.google.com/android/repository/platform-tools-latest-linux.zip",
    "darwin": "https://dl.google.com/android/repository/platform-tools-latest-darwin.zip",
}
SCRCPY_API_URL = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest"


def run_cmd(cmd, cwd=None, show_output=False):
    if show_output:
        print("> " + " ".join(map(str, cmd)))

    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)

    if show_output:
        if result.stdout:
            print(result.stdout.strip())
        if result.stderr:
            print(result.stderr.strip())

    return result


def _find_exe(filename):
    local_path = next(DOWNLOADS_DIR.glob(f"**/{filename}"), None)
    if local_path:
        return local_path
    path = shutil.which(filename)
    if path:
        return Path(path)
    return None


def _download_file(url, dest):
    with requests.get(url, stream=True, timeout=30) as response:
        response.raise_for_status()
        with open(dest, "wb") as handle:
            for chunk in response.iter_content(chunk_size=8192):
                if chunk:
                    handle.write(chunk)


def _extract_archive(archive_path):
    name = archive_path.name.lower()
    if name.endswith(".zip"):
        import zipfile

        with zipfile.ZipFile(archive_path, "r") as zf:
            zf.extractall(DOWNLOADS_DIR)
        return
    if name.endswith(".tar.gz") or name.endswith(".tgz"):
        import tarfile

        with tarfile.open(archive_path, "r:gz") as tf:
            tf.extractall(DOWNLOADS_DIR)
        return
    raise ValueError(f"Unsupported archive format: {archive_path.name}")


def _download_and_extract(name, url, archive_name=None):
    archive_name = archive_name or f"{name}.zip"
    archive_path = DOWNLOADS_DIR / archive_name
    _download_file(url, archive_path)
    _extract_archive(archive_path)
    archive_path.unlink(missing_ok=True)


def _platform_key():
    system = platform.system().lower()
    if system.startswith("win"):
        return "windows"
    if system.startswith("darwin"):
        return "darwin"
    if system.startswith("linux"):
        return "linux"
    return system


def _is_64bit():
    return platform.architecture()[0] == "64bit"


def _arch_tokens():
    machine = platform.machine().lower()
    if machine in {"amd64", "x86_64", "x64"}:
        return ["x86_64", "amd64"]
    if machine in {"arm64", "aarch64"}:
        return ["arm64", "aarch64"]
    if machine in {"armv7l", "armv7"}:
        return ["armv7", "armv7l"]
    if machine in {"i386", "i686", "x86"}:
        return ["x86", "i386", "i686"]
    return [machine]


def _select_scrcpy_asset(assets):
    system = _platform_key()
    candidates = []

    for asset in assets:
        name = (asset.get("name") or "").lower()
        if not name or "server" in name:
            continue
        if system == "windows" and "scrcpy-win" in name:
            if name.endswith(".zip"):
                candidates.append(asset)
        elif system == "linux" and "scrcpy-linux" in name:
            if name.endswith(".tar.gz") or name.endswith(".tgz") or name.endswith(".zip"):
                candidates.append(asset)
        elif system == "darwin" and ("scrcpy-macos" in name or "scrcpy-mac" in name):
            candidates.append(asset)

    if not candidates:
        return None

    if system == "windows":
        target = "win64" if _is_64bit() else "win32"
        for asset in candidates:
            if target in (asset.get("name") or "").lower():
                return asset
        return candidates[0]

    arch_tokens = _arch_tokens()
    for token in arch_tokens:
        for asset in candidates:
            if token in (asset.get("name") or "").lower():
                return asset

    return candidates[0]


def _fetch_scrcpy_release():
    response = requests.get(SCRCPY_API_URL, timeout=10)
    response.raise_for_status()
    payload = response.json()
    assets = []
    for asset in payload.get("assets", []) or []:
        assets.append({
            "name": asset.get("name"),
            "url": asset.get("browser_download_url"),
        })
    return assets


def _verify_tool(path, args):
    try:
        result = run_cmd([str(path)] + args, show_output=False)
        return result.returncode == 0
    except Exception:
        return False


def ensure_tools():
    DOWNLOADS_DIR.mkdir(exist_ok=True)

    adb_name = "adb.exe" if os.name == "nt" else "adb"
    scrcpy_name = "scrcpy.exe" if os.name == "nt" else "scrcpy"

    adb_path = _find_exe(adb_name)
    scrcpy_path = _find_exe(scrcpy_name)

    if not adb_path or not _verify_tool(adb_path, ["version"]):
        platform_key = _platform_key()
        platform_url = PLATFORM_TOOLS_URLS.get(platform_key)
        if not platform_url:
            raise RuntimeError(f"Unsupported platform for adb: {platform_key}")
        _download_and_extract("platform-tools", platform_url)
        adb_path = _find_exe(adb_name)
        if not adb_path:
            raise FileNotFoundError(f"{adb_name} not found after download")

    if not scrcpy_path or not _verify_tool(scrcpy_path, ["--version"]):
        assets = _fetch_scrcpy_release()
        asset = _select_scrcpy_asset(assets)
        if not asset or not asset.get("url"):
            raise FileNotFoundError("scrcpy archive not found for this platform")
        archive_name = asset.get("name") or "scrcpy.zip"
        lowered = archive_name.lower()
        if not (lowered.endswith(".zip") or lowered.endswith(".tar.gz") or lowered.endswith(".tgz")):
            raise RuntimeError(f"Unsupported scrcpy archive format: {archive_name}")
        _download_and_extract("scrcpy", asset["url"], archive_name=archive_name)
        scrcpy_path = _find_exe(scrcpy_name)
        if not scrcpy_path:
            raise FileNotFoundError(f"{scrcpy_name} not found after download")

    return adb_path, scrcpy_path


def start_adb_server(adb_path):
    run_cmd([str(adb_path), "kill-server"], show_output=False)
    run_cmd([str(adb_path), "start-server"], show_output=False)


def stop_adb_server(adb_path):
    run_cmd([str(adb_path), "kill-server"], show_output=False)


def get_connected_devices(adb_path):
    result = run_cmd([str(adb_path), "devices"], show_output=False)
    lines = result.stdout.strip().split("\n")[1:]

    devices = []
    for line in lines:
        if "\t" in line:
            serial, status = line.split("\t")
            devices.append({"serial": serial, "status": status})

    return devices


def adb_shell_get(adb_path, prop):
    result = run_cmd([str(adb_path), "shell", "getprop", prop], show_output=False)
    if result.returncode != 0:
        return None
    value = result.stdout.strip()
    return value or None


def get_device_info(adb_path):
    return {
        "model": adb_shell_get(adb_path, "ro.product.model"),
        "android_version": adb_shell_get(adb_path, "ro.build.version.release"),
    }


def get_device_wifi_ip(adb_path):
    result = run_cmd([str(adb_path), "shell", "ip", "addr", "show", "wlan0"], show_output=False)
    if result.returncode != 0:
        return None

    for line in result.stdout.split("\n"):
        if "inet " in line and "inet6" not in line:
            parts = line.strip().split()
            if len(parts) >= 2:
                return parts[1].split("/")[0]
    return None


def get_setting(adb_path, namespace, key):
    result = run_cmd([str(adb_path), "shell", "settings", "get", namespace, key], show_output=False)
    if result.returncode != 0:
        return None
    value = result.stdout.strip()
    return value if value != "null" else None


def put_setting(adb_path, namespace, key, value):
    run_cmd([str(adb_path), "shell", "settings", "put", namespace, key, str(value)], show_output=False)


def delete_setting(adb_path, namespace, key):
    run_cmd([str(adb_path), "shell", "settings", "delete", namespace, key], show_output=False)
