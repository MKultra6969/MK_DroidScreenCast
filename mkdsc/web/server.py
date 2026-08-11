import asyncio
import json
import platform
import threading
import subprocess
import signal
import zipfile
from contextlib import asynccontextmanager
from datetime import datetime
from pathlib import Path
import re
from typing import List
import time

import requests
import uvicorn
from fastapi import FastAPI, HTTPException, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from starlette.background import BackgroundTask
from starlette.concurrency import run_in_threadpool

from mkdsc.constants import VERSION
from mkdsc.config import (
    load_config,
    save_config,
    replace_config,
    update_config as apply_config_patch,
)
from mkdsc.devices import list_devices, remove_device, save_device
from mkdsc.i18n import available_languages
from mkdsc.i18n.lexicon_web import LEXICON_WEB
from mkdsc.logging_utils import init_logging
from mkdsc.paths import (
    CONFIG_PATH,
    STATIC_DIR,
    TEMPLATES_DIR,
    get_logs_dir,
    get_recordings_dir,
)
from mkdsc.tools import (
    delete_setting,
    ensure_tools,
    get_bootstrap_status,
    get_connected_devices,
    get_device_wifi_ip,
    get_setting,
    put_setting,
    restart_adb_server,
    run_cmd,
    start_adb_server,
)
from mkdsc.updater import check_for_updates
from mkdsc.web.security import (
    CORS_ORIGIN_REGEX,
    TOKEN_HEADER,
    extract_token,
    get_api_token,
    is_allowed_host,
    is_allowed_origin,
    token_matches,
)
from mkdsc.web.service_commands import router as service_router
from mkdsc.web.connection_optimizer import router as connection_router
from mkdsc.web.gallery import router as gallery_router
from mkdsc.web.file_manager import router as file_manager_router

# Пути, которым токен не нужен: сама страница и её статика.
PUBLIC_PATH_PREFIXES = ("/static",)
PUBLIC_PATHS = {"/", "/favicon.ico", "/index.html"}

RECORDING_STOP_TIMEOUT = 45


async def _bootstrap_tools(app: FastAPI):
    """Готовит adb/scrcpy в фоне.

    Раньше это происходило в startup-хуке: uvicorn не принимал соединения,
    пока не скачает ~150 МБ, а при ошибке сети не стартовал вовсе.
    """
    app.state.bootstrap_error = None
    try:
        adb_path, scrcpy_path = await asyncio.to_thread(ensure_tools)
        app.state.adb_path = adb_path
        app.state.scrcpy_path = scrcpy_path
        await asyncio.to_thread(start_adb_server, adb_path)
        app.state.logger.info("tools ready: adb=%s scrcpy=%s", adb_path, scrcpy_path)
    except Exception as exc:
        app.state.bootstrap_error = str(exc)
        app.state.logger.exception("tool bootstrap failed")


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.config = load_config()
    app.state.logger, _ = init_logging("web", app.state.config)
    app.state.logger.info("version: %s", VERSION)
    app.state.logger.info("start: %s", datetime.now().isoformat())

    app.state.adb_path = None
    app.state.scrcpy_path = None
    app.state.bootstrap_error = None
    app.state.recording = None
    app.state.recording_last_error = None
    app.state.bind_host = getattr(app.state, "bind_host", "")

    bootstrap_task = asyncio.create_task(_bootstrap_tools(app))

    yield

    bootstrap_task.cancel()
    try:
        await bootstrap_task
    except (asyncio.CancelledError, Exception):
        pass


app = FastAPI(title="MK DroidScreenCast Web Panel", lifespan=lifespan)

# Подключаем роутеры новых модулей
app.include_router(service_router)
app.include_router(connection_router)
app.include_router(gallery_router)
app.include_router(file_manager_router)


@app.middleware("http")
async def log_exceptions(request, call_next):
    try:
        return await call_next(request)
    except Exception as exc:
        logger = getattr(app.state, "logger", None)
        if logger:
            logger.exception("Unhandled error: %s %s", request.method, request.url.path)
        return JSONResponse(status_code=500, content={"detail": str(exc)})


def _needs_token(path: str) -> bool:
    if path in PUBLIC_PATHS:
        return False
    return not any(path.startswith(prefix) for prefix in PUBLIC_PATH_PREFIXES)


@app.middleware("http")
async def enforce_local_access(request, call_next):
    """Origin + Host + токен.

    Без этого любая открытая в браузере вкладка могла читать, скачивать и
    удалять файлы подключённого телефона через fetch на 127.0.0.1:6969.
    """
    if request.method == "OPTIONS":
        return await call_next(request)

    if not is_allowed_host(request.headers.get("host", ""), getattr(app.state, "bind_host", "")):
        return JSONResponse(status_code=403, content={"detail": "Host header not allowed"})

    if not is_allowed_origin(request.headers.get("origin", "")):
        return JSONResponse(status_code=403, content={"detail": "Origin not allowed"})

    if _needs_token(request.url.path):
        token = extract_token(request.headers, request.query_params)
        if not token_matches(token):
            return JSONResponse(
                status_code=401,
                content={"detail": "Missing or invalid API token"},
            )

    return await call_next(request)


# Добавляется последним => выполняется первым и сам отвечает на preflight.
app.add_middleware(
    CORSMiddleware,
    allow_origin_regex=CORS_ORIGIN_REGEX,
    allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allow_headers=["Content-Type", TOKEN_HEADER],
)

STATIC_DIR.mkdir(parents=True, exist_ok=True)
TEMPLATES_DIR.mkdir(parents=True, exist_ok=True)

app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")


def _get_config():
    return load_config()


def _save_config(config):
    save_config(config)


def _require_adb():
    adb_path = getattr(app.state, "adb_path", None)
    if adb_path:
        return adb_path
    detail = getattr(app.state, "bootstrap_error", None) or "adb is still being prepared"
    raise HTTPException(status_code=503, detail=detail)


def _require_scrcpy():
    scrcpy_path = getattr(app.state, "scrcpy_path", None)
    if scrcpy_path:
        return scrcpy_path
    detail = getattr(app.state, "bootstrap_error", None) or "scrcpy is still being prepared"
    raise HTTPException(status_code=503, detail=detail)


def _tool_version(tool_path, args):
    if not tool_path:
        return "not available"
    return run_cmd([str(tool_path)] + args, show_output=False).stdout


def _create_logs_zip(zip_path: Path):
    adb_version = _tool_version(getattr(app.state, "adb_path", None), ["version"])
    scrcpy_version = _tool_version(getattr(app.state, "scrcpy_path", None), ["--version"])
    logs_dir = get_logs_dir()

    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as archive:
        for log_file in logs_dir.glob("*.log"):
            archive.write(log_file, f"logs/{log_file.name}")

        if CONFIG_PATH.exists():
            archive.write(CONFIG_PATH, "config.json")

        archive.writestr("version.txt", VERSION)
        archive.writestr("adb_version.txt", adb_version)
        archive.writestr("scrcpy_version.txt", scrcpy_version)


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


@app.get("/", response_class=HTMLResponse)
async def get_index():
    static_index = STATIC_DIR / "index.html"
    html_file = static_index if static_index.exists() else TEMPLATES_DIR / "index.html"
    if not html_file.exists():
        return HTMLResponse("<h1>Missing templates/index.html</h1>")

    html = html_file.read_text(encoding="utf-8")
    # Токен уходит только в саму страницу: сторонний сайт её не прочитает.
    injection = f"<script>window.__MKDSC_TOKEN__={json.dumps(get_api_token())};</script>"
    if "<head>" in html:
        html = html.replace("<head>", "<head>" + injection, 1)
    else:
        html = injection + html
    return HTMLResponse(html)


@app.get("/api/bootstrap/status")
async def bootstrap_status():
    ready = bool(getattr(app.state, "adb_path", None) and getattr(app.state, "scrcpy_path", None))
    error = getattr(app.state, "bootstrap_error", None)
    progress = get_bootstrap_status()
    return {
        "ready": ready,
        "error": error,
        "stage": "ready" if ready else ("error" if error else progress.get("stage", "starting")),
        "tool": progress.get("tool", ""),
        "downloaded_bytes": progress.get("downloaded_bytes", 0),
        "total_bytes": progress.get("total_bytes", 0),
    }


@app.get("/api/config")
async def get_config():
    config = _get_config()
    return {
        "language": config.get("language", "en"),
        "presets": config.get("scrcpy", {}).get("presets", []),
        "web": config.get("web", {}),
        "logs": config.get("logs", {}),
        "downloads": config.get("downloads", {}),
        "connection_optimizer": config.get("connection_optimizer", {}),
        "recording": config.get("recording", {}),
        "version": VERSION,
        "languages": available_languages(LEXICON_WEB),
    }


@app.get("/api/config/full")
async def get_config_full():
    return _get_config()


@app.get("/api/update/check")
async def api_check_updates():
    return await asyncio.to_thread(check_for_updates)


@app.post("/api/config")
async def update_config(data: dict):
    """Частичный патч: присланные ключи сливаются с текущим конфигом."""
    if not isinstance(data, dict):
        raise HTTPException(status_code=400, detail="Config payload required")
    try:
        config = apply_config_patch(data)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    return {"success": True, "config": config}


@app.put("/api/config")
async def replace_config_endpoint(data: dict):
    """Полная замена: удалённые в редакторе ключи действительно исчезают.

    POST со слиянием не умел удалять — пользователь стирал ключ, видел
    «Config saved», а ключ возвращался.
    """
    if not isinstance(data, dict):
        raise HTTPException(status_code=400, detail="Config payload required")
    try:
        config = replace_config(data)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    return {"success": True, "config": config}


@app.get("/api/i18n")
async def get_i18n(lang: str = "en"):
    base = LEXICON_WEB.get("en", {})
    localized = LEXICON_WEB.get(lang, base)
    if lang == "en":
        strings = base
    else:
        strings = {**base, **localized}
    return {
        "language": lang,
        "strings": strings,
    }


@app.get("/api/devices")
async def get_devices():
    saved = await run_in_threadpool(list_devices)
    adb_path = getattr(app.state, "adb_path", None)
    connected = []
    if adb_path:
        connected = await run_in_threadpool(get_connected_devices, adb_path)

    return {
        "saved": saved,
        "connected": connected,
        "timestamp": datetime.now().isoformat(),
    }


@app.post("/api/adb/restart")
async def restart_adb():
    """Явный перезапуск adb-сервера (кнопка в диагностике)."""
    adb_path = _require_adb()
    await run_in_threadpool(restart_adb_server, adb_path)
    return {"success": True}


@app.post("/api/connect")
async def connect_device(data: dict):
    address = data.get("address")

    if not address:
        raise HTTPException(status_code=400, detail="Address required")

    adb_path = _require_adb()
    result = await run_in_threadpool(
        run_cmd, [str(adb_path), "connect", address], None, False, 15
    )

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

    adb_path = _require_adb()
    result = await run_in_threadpool(
        run_cmd, [str(adb_path), "disconnect", address], None, False, 15
    )

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

    await run_in_threadpool(save_device, name, ip, port, connection_type)

    return {"success": True, "message": f"Device '{name}' saved"}


@app.delete("/api/devices/{ip}/{port}")
async def delete_device(ip: str, port: str):
    await run_in_threadpool(remove_device, ip, port)
    return {"success": True}


def _run_pair(adb_path, pair_address, pair_code, timeout=30):
    proc = subprocess.Popen(
        [str(adb_path), "pair", pair_address],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        output, _ = proc.communicate(input=pair_code + "\n", timeout=timeout)
    except subprocess.TimeoutExpired:
        # Без таймаута adb pair на недоступном адресе вешал весь бэкенд.
        proc.kill()
        try:
            output, _ = proc.communicate(timeout=5)
        except Exception:
            output = ""
        output = (output or "") + f"\n[adb pair timed out after {timeout} seconds]"
    return output


@app.post("/api/pair")
async def pair_device(data: dict):
    pair_address = data.get("pair_address")
    pair_code = data.get("pair_code")

    if not all([pair_address, pair_code]):
        raise HTTPException(status_code=400, detail="Pair address and code required")

    adb_path = _require_adb()
    output = await run_in_threadpool(_run_pair, adb_path, pair_address, pair_code, 30)
    success = "Successfully paired" in output

    return {
        "success": success,
        "output": output,
    }


@app.post("/api/tcpip")
async def enable_tcpip(data: dict):
    port = data.get("port", "5555")
    serial = data.get("serial")

    adb_path = _require_adb()
    cmd = [str(adb_path)]
    if serial:
        cmd.extend(["-s", str(serial)])
    cmd.extend(["tcpip", str(port)])

    result = await run_in_threadpool(run_cmd, cmd, None, False, 15)
    success = result.returncode == 0

    ip_address = None
    if success:
        ip_address = await run_in_threadpool(get_device_wifi_ip, adb_path, serial)

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


def _apply_device_settings(adb_path, stay_awake, show_touches, serial=None):
    """Возвращает (что восстановить, список незаданных настроек).

    Серийник обязателен: при двух подключённых устройствах adb отвечает
    'more than one device', и настройки молча не применялись.
    """
    restore = {}
    failed = []

    if stay_awake:
        previous = get_setting(adb_path, "global", "stay_on_while_plugged_in", serial)
        if put_setting(adb_path, "global", "stay_on_while_plugged_in", 3, serial):
            restore["global:stay_on_while_plugged_in"] = previous
        else:
            failed.append("stay_awake")

    if show_touches:
        previous = get_setting(adb_path, "system", "show_touches", serial)
        if put_setting(adb_path, "system", "show_touches", 1, serial):
            restore["system:show_touches"] = previous
        else:
            failed.append("show_touches")

    return restore, failed


def _restore_device_settings(adb_path, restore, serial=None):
    for key, value in restore.items():
        namespace, setting = key.split(":", 1)
        if value is None:
            delete_setting(adb_path, namespace, setting, serial)
        else:
            put_setting(adb_path, namespace, setting, value, serial)


def _restore_after(proc, adb_path, restore, logger, serial=None):
    proc.wait()
    if restore:
        _restore_device_settings(adb_path, restore, serial)
    logger.info("scrcpy exited with code %s", proc.returncode)


def _normalize_keyboard_mode(keyboard: str):
    warning_key = None
    if keyboard == "aoa" and platform.system().lower().startswith("win"):
        keyboard = "uhid"
        warning_key = "notification_aoa_windows_fallback"
    return keyboard, warning_key


def _sanitize_prefix(prefix: str) -> str:
    prefix = (prefix or "").strip()
    if not prefix:
        return "recording"
    cleaned = []
    for ch in prefix:
        if ("a" <= ch <= "z") or ("A" <= ch <= "Z") or ("0" <= ch <= "9") or ch in ("-", "_"):
            cleaned.append(ch)
        else:
            cleaned.append("_")
    text = "".join(cleaned)
    text = re.sub(r"_+", "_", text).strip("_")
    return text or "recording"


def _get_recording_session():
    session = getattr(app.state, "recording", None)
    if not session:
        return None
    proc = session.get("process")
    if not proc:
        app.state.recording = None
        return None
    if proc.poll() is None:
        return session
    app.state.recording = None
    return None


def _recording_status_payload(session=None):
    session = session or _get_recording_session()
    if not session:
        payload = {"active": False}
        last_error = getattr(app.state, "recording_last_error", None)
        if last_error:
            payload["last_error"] = last_error
        return payload
    settings = session.get("settings", {})
    return {
        "active": True,
        "pid": session.get("pid"),
        "started_at": session.get("started_at"),
        "output_path": session.get("output_path"),
        "format": settings.get("format"),
        "audio_source": settings.get("audio_source"),
        "show_preview": settings.get("show_preview"),
        "serial": settings.get("serial"),
        "connection": settings.get("connection"),
    }


@app.post("/api/scrcpy/launch")
async def launch_scrcpy_api(data: dict):
    scrcpy_path = _require_scrcpy()
    adb_path = _require_adb()
    cmd = [str(scrcpy_path)]

    if data.get("bitrate"):
        cmd.extend(["--video-bit-rate", data["bitrate"]])
    if data.get("maxsize"):
        cmd.extend(["--max-size", data["maxsize"]])

    keyboard = data.get("keyboard", "uhid")
    keyboard, warning_key = _normalize_keyboard_mode(keyboard)
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

    restore, failed_settings = await run_in_threadpool(
        _apply_device_settings, adb_path, stay_awake, show_touches, serial
    )

    logger = app.state.logger

    try:
        proc = subprocess.Popen(cmd)
    except Exception as exc:
        if restore:
            await run_in_threadpool(_restore_device_settings, adb_path, restore, serial)
        return {"success": False, "output": str(exc)}

    threading.Thread(
        target=_restore_after,
        args=(proc, adb_path, restore, logger, serial),
        daemon=True,
    ).start()

    logger.info("scrcpy settings: %s", data)

    return {
        "success": True,
        "pid": proc.pid,
        "command": " ".join(cmd),
        "warning_key": warning_key,
        "failed_settings": failed_settings,
    }


@app.get("/api/recording/status")
async def recording_status():
    return _recording_status_payload()


def _recording_watch(proc, adb_path, restore, logger, serial=None):
    output_lines = []
    if proc.stdout:
        for line in proc.stdout:
            message = line.strip()
            if message:
                logger.info("recording: %s", message)
                output_lines.append(message)
                if len(output_lines) > 40:
                    output_lines.pop(0)
    proc.wait()
    if restore:
        _restore_device_settings(adb_path, restore, serial)
    logger.info("recording exited with code %s", proc.returncode)
    session = getattr(app.state, "recording", None)
    if session and session.get("process") == proc:
        stopping = bool(session.get("stopping"))
        app.state.recording = None
        if proc.returncode != 0 and not stopping:
            last_error = {
                "exit_code": proc.returncode,
                "timestamp": datetime.now().isoformat(),
            }
            if output_lines:
                last_error["output"] = "\n".join(output_lines[-10:])
            app.state.recording_last_error = last_error


@app.post("/api/recording/start")
async def start_recording(data: dict):
    if _get_recording_session():
        raise HTTPException(status_code=409, detail="Recording already active")

    scrcpy_path = _require_scrcpy()
    adb_path = _require_adb()

    app.state.recording_last_error = None
    config = _get_config()
    recording_cfg = config.get("recording", {})

    output_dir = data.get("output_dir") or recording_cfg.get("output_dir") or str(
        get_recordings_dir(config)
    )
    output_dir_path = Path(output_dir).expanduser()
    try:
        output_dir_path.mkdir(parents=True, exist_ok=True)
    except Exception as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    if not output_dir_path.is_dir():
        raise HTTPException(status_code=400, detail="Output path is not a directory")

    fmt = (data.get("format") or recording_cfg.get("format") or "mp4").lower()
    if fmt not in {"mp4", "mkv"}:
        raise HTTPException(status_code=400, detail="Unsupported format")

    prefix = _sanitize_prefix(data.get("file_prefix") or recording_cfg.get("file_prefix"))
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    filename = f"{prefix}_{timestamp}.{fmt}"
    output_path = output_dir_path / filename

    bitrate = data.get("bitrate") or config.get("scrcpy", {}).get("bitrate", "8M")
    maxsize = data.get("maxsize") or config.get("scrcpy", {}).get("maxsize", "1080")
    keyboard = data.get("keyboard") or config.get("scrcpy", {}).get("keyboard", "uhid")
    keyboard, warning_key = _normalize_keyboard_mode(keyboard or "uhid")

    cmd = [str(scrcpy_path)]
    cmd.extend(["--record", str(output_path)])
    if bitrate:
        cmd.extend(["--video-bit-rate", str(bitrate)])
    if maxsize:
        cmd.extend(["--max-size", str(maxsize)])
    if keyboard:
        cmd.append(f"--keyboard={keyboard}")

    serial = data.get("serial")
    if serial:
        cmd.extend(["--serial", serial])
    else:
        connection = data.get("connection")
        if connection == "usb":
            cmd.append("--select-usb")
        elif connection == "wifi":
            cmd.append("--select-tcpip")

    if "turn_screen_off" in data:
        turn_screen_off = bool(data.get("turn_screen_off"))
    else:
        turn_screen_off = bool(recording_cfg.get("turn_screen_off"))
    if turn_screen_off:
        cmd.append("--turn-screen-off")

    show_preview = data.get("show_preview")
    if show_preview is None:
        show_preview = recording_cfg.get("show_preview", True)
    if not show_preview:
        cmd.append("--no-window")
        cmd.append("--no-audio-playback")

    audio_source = (data.get("audio_source") or recording_cfg.get("audio_source") or "output").lower()
    if audio_source in {"none", "off"}:
        cmd.append("--no-audio")
    else:
        cmd.append(f"--audio-source={audio_source}")

    if "stay_awake" in data:
        stay_awake = bool(data.get("stay_awake"))
    else:
        stay_awake = bool(recording_cfg.get("stay_awake", False))

    if "show_touches" in data:
        show_touches = bool(data.get("show_touches"))
    else:
        show_touches = bool(recording_cfg.get("show_touches", False))
    restore, failed_settings = await run_in_threadpool(
        _apply_device_settings, adb_path, stay_awake, show_touches, serial
    )

    logger = app.state.logger

    try:
        creationflags = 0
        if platform.system().lower().startswith("win"):
            creationflags = subprocess.CREATE_NEW_PROCESS_GROUP
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            creationflags=creationflags,
        )
    except Exception as exc:
        if restore:
            await run_in_threadpool(_restore_device_settings, adb_path, restore, serial)
        raise HTTPException(status_code=500, detail=str(exc)) from exc

    await asyncio.sleep(0.4)
    if proc.poll() is not None:
        output = ""
        if proc.stdout:
            try:
                output, _ = await run_in_threadpool(proc.communicate, None, 1)
            except Exception:
                output = ""
        if restore:
            await run_in_threadpool(_restore_device_settings, adb_path, restore, serial)
        logger.info("recording failed to start: exit code %s", proc.returncode)
        if output:
            logger.info("recording output: %s", output.strip())
        detail = output.strip() if output else f"Recording failed to start (exit code {proc.returncode})."
        app.state.recording_last_error = {
            "exit_code": proc.returncode,
            "timestamp": datetime.now().isoformat(),
            "output": detail,
        }
        raise HTTPException(status_code=500, detail=detail)

    settings = {
        "format": fmt,
        "audio_source": audio_source,
        "show_preview": show_preview,
        "serial": serial,
        "connection": data.get("connection"),
    }

    app.state.recording = {
        "process": proc,
        "pid": proc.pid,
        "started_at": datetime.now().isoformat(),
        "output_path": str(output_path),
        "settings": settings,
        "restore": restore,
        "stopping": False,
    }

    threading.Thread(
        target=_recording_watch,
        args=(proc, adb_path, restore, logger, serial),
        daemon=True,
    ).start()

    logger.info("recording settings: %s", data)
    logger.info("recording command: %s", " ".join(cmd))

    return {
        "success": True,
        "pid": proc.pid,
        "started_at": app.state.recording.get("started_at"),
        "output_path": str(output_path),
        "filename": filename,
        "settings": settings,
        "warning_key": warning_key,
        "failed_settings": failed_settings,
    }


def _stop_recording_process(proc, timeout=RECORDING_STOP_TIMEOUT):
    """Даём scrcpy дописать контейнер.

    TerminateProcess на Windows убивает процесс мгновенно — MP4 остаётся без
    moov-атома и не открывается ничем. Поэтому долгий graceful-таймаут и
    честный флаг, если всё-таки пришлось убивать.
    """
    try:
        if platform.system().lower().startswith("win"):
            proc.send_signal(signal.CTRL_BREAK_EVENT)
        else:
            proc.send_signal(signal.SIGINT)
    except Exception:
        pass

    try:
        proc.wait(timeout=timeout)
        return True
    except Exception:
        pass

    try:
        proc.terminate()
        proc.wait(timeout=5)
    except Exception:
        try:
            proc.kill()
            proc.wait(timeout=5)
        except Exception:
            pass
    return False


@app.post("/api/recording/stop")
async def stop_recording():
    session = _get_recording_session()
    if not session:
        return {"success": False, "message": "No active recording"}

    proc = session.get("process")
    if not proc:
        app.state.recording = None
        return {"success": False, "message": "Recording process missing"}

    session["stopping"] = True
    output_path = session.get("output_path")
    graceful = await run_in_threadpool(_stop_recording_process, proc, RECORDING_STOP_TIMEOUT)

    return {
        "success": True,
        "graceful": graceful,
        "output_path": output_path,
        "warning_key": None if graceful else "notification_recording_forced_stop",
    }


@app.get("/api/logs/download")
async def download_logs():
    zip_name = f"logs_{datetime.now().strftime('%Y%m%d_%H%M%S')}.zip"
    zip_path = get_logs_dir() / zip_name

    await run_in_threadpool(_create_logs_zip, zip_path)

    def _cleanup(path: Path):
        path.unlink(missing_ok=True)

    return FileResponse(
        zip_path,
        filename="logs.zip",
        media_type="application/zip",
        background=BackgroundTask(_cleanup, zip_path),
    )


@app.post("/api/logs/export")
async def export_logs(data: dict):
    directory = (data or {}).get("directory")
    if not directory:
        raise HTTPException(status_code=400, detail="Directory required")

    target_dir = Path(directory).expanduser()
    try:
        target_dir.mkdir(parents=True, exist_ok=True)
    except Exception as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    if not target_dir.is_dir():
        raise HTTPException(status_code=400, detail="Target path is not a directory")

    zip_name = f"logs_{datetime.now().strftime('%Y%m%d_%H%M%S')}.zip"
    zip_path = target_dir / zip_name

    await run_in_threadpool(_create_logs_zip, zip_path)

    return {"success": True, "path": str(zip_path), "filename": zip_name}


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    # HTTP-middleware для WebSocket не выполняется — проверяем здесь.
    if not is_allowed_origin(websocket.headers.get("origin", "")):
        await websocket.close(code=1008)
        return
    token = extract_token(websocket.headers, websocket.query_params)
    if not token_matches(token):
        await websocket.close(code=1008)
        return

    await manager.connect(websocket)

    try:
        while True:
            adb_path = getattr(app.state, "adb_path", None)
            devices = []
            if adb_path:
                devices = await run_in_threadpool(get_connected_devices, adb_path)
            await websocket.send_json({
                "type": "devices_update",
                "devices": devices,
                "timestamp": datetime.now().isoformat(),
            })

            await asyncio.sleep(3)

    except WebSocketDisconnect:
        pass
    except Exception:
        # Любая другая ошибка тоже должна освободить соединение, иначе
        # мёртвые сокеты копились в active_connections навсегда.
        logger = getattr(app.state, "logger", None)
        if logger:
            logger.info("websocket closed with error", exc_info=True)
    finally:
        manager.disconnect(websocket)


def run_server(host=None, port=None, auto_open=None):
    config = _get_config()
    host = host or config.get("web", {}).get("host", "127.0.0.1")
    port = port or config.get("web", {}).get("port", 6969)
    if auto_open is None:
        auto_open = config.get("web", {}).get("auto_open", True)

    app.state.bind_host = host
    # Токен создаётся до старта, чтобы попасть в окружение дочерних процессов.
    get_api_token()

    if auto_open:
        url = f"http://localhost:{port}"
        threading.Thread(target=_open_browser_when_ready, args=(url,), daemon=True).start()

    uvicorn.run(app, host=host, port=port)


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
