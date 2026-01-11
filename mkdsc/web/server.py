import asyncio
import platform
import threading
import subprocess
import zipfile
from datetime import datetime
from pathlib import Path
from typing import List
import time

import requests
import uvicorn
from fastapi import FastAPI, HTTPException, WebSocket, WebSocketDisconnect
from fastapi.responses import FileResponse, HTMLResponse
from fastapi.staticfiles import StaticFiles
from starlette.background import BackgroundTask

from mkdsc.constants import VERSION
from mkdsc.config import load_config, save_config
from mkdsc.devices import list_devices, remove_device, save_device
from mkdsc.i18n import available_languages
from mkdsc.i18n.lexicon_web import LEXICON_WEB
from mkdsc.logging_utils import init_logging
from mkdsc.paths import CONFIG_PATH, LOGS_DIR, STATIC_DIR, TEMPLATES_DIR
from mkdsc.tools import (
    delete_setting,
    ensure_tools,
    get_connected_devices,
    get_device_info,
    get_device_wifi_ip,
    get_setting,
    put_setting,
    run_cmd,
    start_adb_server,
    stop_adb_server,
)
from mkdsc.updater import apply_update, check_for_updates

app = FastAPI(title="MK DroidScreenCast Web Panel")

STATIC_DIR.mkdir(exist_ok=True)
TEMPLATES_DIR.mkdir(exist_ok=True)
LOGS_DIR.mkdir(exist_ok=True)

app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")


def _get_config():
    return load_config()


def _save_config(config):
    save_config(config)


class ConnectionManager:
    def __init__(self):
        self.active_connections: List[WebSocket] = []

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.append(websocket)

    def disconnect(self, websocket: WebSocket):
        if websocket in self.active_connections:
            self.active_connections.remove(websocket)

    async def broadcast(self, message: dict):
        for connection in list(self.active_connections):
            try:
                await connection.send_json(message)
            except Exception:
                self.disconnect(connection)


manager = ConnectionManager()


@app.on_event("startup")
async def startup_event():
    app.state.config = _get_config()
    app.state.logger, _ = init_logging("web")
    app.state.logger.info("version: %s", VERSION)
    app.state.logger.info("start: %s", datetime.now().isoformat())

    app.state.adb_path, app.state.scrcpy_path = ensure_tools()
    start_adb_server(app.state.adb_path)


@app.on_event("shutdown")
async def shutdown_event():
    adb_path = getattr(app.state, "adb_path", None)
    if adb_path:
        stop_adb_server(adb_path)


@app.get("/", response_class=HTMLResponse)
async def get_index():
    html_file = TEMPLATES_DIR / "index.html"
    if html_file.exists():
        return FileResponse(html_file)
    return HTMLResponse("<h1>Missing templates/index.html</h1>")


@app.get("/api/config")
async def get_config():
    config = _get_config()
    return {
        "language": config.get("language", "en"),
        "presets": config.get("scrcpy", {}).get("presets", []),
        "web": config.get("web", {}),
        "version": VERSION,
        "languages": available_languages(LEXICON_WEB),
    }


@app.get("/api/update/check")
async def api_check_updates():
    return await asyncio.to_thread(check_for_updates)


@app.post("/api/update/apply")
async def api_apply_update():
    return await asyncio.to_thread(apply_update)


@app.post("/api/config")
async def update_config(data: dict):
    config = _get_config()

    if "language" in data:
        config["language"] = data["language"]
    if "presets" in data:
        config.setdefault("scrcpy", {})["presets"] = data["presets"]

    _save_config(config)
    return {"success": True, "config": config}


@app.get("/api/i18n")
async def get_i18n(lang: str = "en"):
    return {
        "language": lang,
        "strings": LEXICON_WEB.get(lang, LEXICON_WEB.get("en", {})),
    }


@app.get("/api/devices")
async def get_devices():
    saved = list_devices()
    connected = get_connected_devices(app.state.adb_path)

    return {
        "saved": saved,
        "connected": connected,
        "timestamp": datetime.now().isoformat(),
    }


@app.post("/api/connect")
async def connect_device(data: dict):
    address = data.get("address")

    if not address:
        raise HTTPException(status_code=400, detail="Address required")

    result = run_cmd([str(app.state.adb_path), "connect", address], show_output=False)

    success = result.returncode == 0 and "connected" in result.stdout.lower()

    await manager.broadcast({
        "type": "device_status_changed",
        "address": address,
        "connected": success,
    })

    return {
        "success": success,
        "output": result.stdout + result.stderr,
        "address": address,
    }


@app.post("/api/disconnect")
async def disconnect_device(data: dict):
    address = data.get("address")

    result = run_cmd([str(app.state.adb_path), "disconnect", address], show_output=False)

    await manager.broadcast({
        "type": "device_status_changed",
        "address": address,
        "connected": False,
    })

    return {"success": True, "output": result.stdout}


@app.post("/api/devices/save")
async def save_device_endpoint(data: dict):
    name = data.get("name")
    ip = data.get("ip")
    port = data.get("port", "5555")
    connection_type = data.get("type", "wifi")

    if not all([name, ip]):
        raise HTTPException(status_code=400, detail="Name and IP required")

    save_device(name, ip, port, connection_type)

    return {"success": True, "message": f"Device '{name}' saved"}


@app.delete("/api/devices/{ip}/{port}")
async def delete_device(ip: str, port: str):
    remove_device(ip, port)
    return {"success": True}


@app.post("/api/pair")
async def pair_device(data: dict):
    pair_address = data.get("pair_address")
    pair_code = data.get("pair_code")

    if not all([pair_address, pair_code]):
        raise HTTPException(status_code=400, detail="Pair address and code required")

    proc = subprocess.Popen(
        [str(app.state.adb_path), "pair", pair_address],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output, _ = proc.communicate(input=pair_code + "\n")
    success = "Successfully paired" in output

    return {
        "success": success,
        "output": output,
    }


@app.post("/api/tcpip")
async def enable_tcpip(data: dict):
    port = data.get("port", "5555")

    result = run_cmd([str(app.state.adb_path), "tcpip", port], show_output=False)
    success = result.returncode == 0

    ip_address = None
    if success:
        ip_address = get_device_wifi_ip(app.state.adb_path)

    return {
        "success": success,
        "ip": ip_address,
        "port": port,
        "output": result.stdout + result.stderr,
    }


@app.get("/api/presets")
async def get_presets():
    config = _get_config()
    return config.get("scrcpy", {}).get("presets", [])


@app.post("/api/presets")
async def save_preset(data: dict):
    name = data.get("name")
    bitrate = data.get("bitrate")
    maxsize = data.get("maxsize")

    if not name:
        raise HTTPException(status_code=400, detail="Name required")

    config = _get_config()
    presets = config.setdefault("scrcpy", {}).setdefault("presets", [])

    for preset in presets:
        if preset.get("name", "").lower() == name.lower():
            preset.update({"bitrate": bitrate, "maxsize": maxsize})
            break
    else:
        presets.append({"name": name, "bitrate": bitrate, "maxsize": maxsize})

    _save_config(config)
    return {"success": True, "presets": presets}


@app.delete("/api/presets/{name}")
async def delete_preset(name: str):
    config = _get_config()
    presets = config.setdefault("scrcpy", {}).setdefault("presets", [])

    presets = [preset for preset in presets if preset.get("name", "").lower() != name.lower()]
    config["scrcpy"]["presets"] = presets
    _save_config(config)

    return {"success": True, "presets": presets}


def _apply_device_settings(adb_path, stay_awake, show_touches):
    restore = {}

    if stay_awake:
        previous = get_setting(adb_path, "global", "stay_on_while_plugged_in")
        restore["global:stay_on_while_plugged_in"] = previous
        put_setting(adb_path, "global", "stay_on_while_plugged_in", 3)

    if show_touches:
        previous = get_setting(adb_path, "system", "show_touches")
        restore["system:show_touches"] = previous
        put_setting(adb_path, "system", "show_touches", 1)

    return restore


def _restore_device_settings(adb_path, restore):
    for key, value in restore.items():
        namespace, setting = key.split(":", 1)
        if value is None:
            delete_setting(adb_path, namespace, setting)
        else:
            put_setting(adb_path, namespace, setting, value)


def _restore_after(proc, adb_path, restore, logger):
    proc.wait()
    if restore:
        _restore_device_settings(adb_path, restore)
    logger.info("scrcpy exited with code %s", proc.returncode)


@app.post("/api/scrcpy/launch")
async def launch_scrcpy_api(data: dict):
    cmd = [str(app.state.scrcpy_path)]

    if data.get("bitrate"):
        cmd.extend(["--video-bit-rate", data["bitrate"]])
    if data.get("maxsize"):
        cmd.extend(["--max-size", data["maxsize"]])

    keyboard = data.get("keyboard", "uhid")
    warning_key = None
    if keyboard == "aoa" and platform.system().lower().startswith("win"):
        keyboard = "uhid"
        warning_key = "notification_aoa_windows_fallback"
    cmd.append(f"--keyboard={keyboard}")

    serial = data.get("serial")
    if serial:
        cmd.extend(["--serial", serial])
    else:
        if data.get("connection") == "usb":
            cmd.append("--select-usb")
        elif data.get("connection") == "wifi":
            cmd.append("--select-tcpip")

    if data.get("turn_screen_off"):
        cmd.append("--turn-screen-off")
    if data.get("fullscreen"):
        cmd.append("--fullscreen")
    if data.get("no_audio"):
        cmd.append("--no-audio")

    stay_awake = data.get("stay_awake", False)
    show_touches = data.get("show_touches", False)

    restore = _apply_device_settings(app.state.adb_path, stay_awake, show_touches)

    logger = app.state.logger

    try:
        proc = subprocess.Popen(cmd)
    except Exception as exc:
        if restore:
            _restore_device_settings(app.state.adb_path, restore)
        return {"success": False, "output": str(exc)}

    threading.Thread(
        target=_restore_after,
        args=(proc, app.state.adb_path, restore, logger),
        daemon=True,
    ).start()

    logger.info("scrcpy settings: %s", data)
    logger.info("device info: %s", get_device_info(app.state.adb_path))
    logger.info("local ip: %s", get_device_wifi_ip(app.state.adb_path))

    return {
        "success": True,
        "pid": proc.pid,
        "command": " ".join(cmd),
        "warning_key": warning_key,
    }


@app.get("/api/logs/download")
async def download_logs():
    zip_name = f"logs_{datetime.now().strftime('%Y%m%d_%H%M%S')}.zip"
    zip_path = LOGS_DIR / zip_name

    adb_version = run_cmd([str(app.state.adb_path), "version"], show_output=False).stdout
    scrcpy_version = run_cmd([str(app.state.scrcpy_path), "--version"], show_output=False).stdout

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as archive:
        for log_file in LOGS_DIR.glob("*.log"):
            archive.write(log_file, f"logs/{log_file.name}")

        if CONFIG_PATH.exists():
            archive.write(CONFIG_PATH, "config.json")

        archive.writestr("version.txt", VERSION)
        archive.writestr("adb_version.txt", adb_version)
        archive.writestr("scrcpy_version.txt", scrcpy_version)

    def _cleanup(path: Path):
        path.unlink(missing_ok=True)

    return FileResponse(
        zip_path,
        filename="logs.zip",
        media_type="application/zip",
        background=BackgroundTask(_cleanup, zip_path),
    )


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await manager.connect(websocket)

    try:
        while True:
            devices = get_connected_devices(app.state.adb_path)
            await websocket.send_json({
                "type": "devices_update",
                "devices": devices,
                "timestamp": datetime.now().isoformat(),
            })

            await asyncio.sleep(3)

    except WebSocketDisconnect:
        manager.disconnect(websocket)


def run_server(host=None, port=None, auto_open=None):
    config = _get_config()
    host = host or config.get("web", {}).get("host", "0.0.0.0")
    port = port or config.get("web", {}).get("port", 6969)
    if auto_open is None:
        auto_open = config.get("web", {}).get("auto_open", True)

    if auto_open:
        url = f"http://localhost:{port}"
        threading.Thread(target=_open_browser_when_ready, args=(url,), daemon=True).start()

    try:
        uvicorn.run(app, host=host, port=port)
    finally:
        adb_path = getattr(app.state, "adb_path", None)
        if adb_path:
            stop_adb_server(adb_path)


def _open_browser(url):
    try:
        import webbrowser

        webbrowser.open(url)
    except Exception:
        pass


def _open_browser_when_ready(url, timeout=15):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            response = requests.get(url, timeout=1)
            if response.status_code < 500:
                _open_browser(url)
                return
        except Exception:
            pass
        time.sleep(0.4)


if __name__ == "__main__":
    run_server()
