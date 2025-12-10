from fastapi import FastAPI, WebSocket, WebSocketDisconnect, HTTPException
from fastapi.responses import HTMLResponse, JSONResponse, FileResponse
from fastapi.staticfiles import StaticFiles
import asyncio
import subprocess
import json
from pathlib import Path
from datetime import datetime
from typing import List, Dict
import uvicorn

from screencast import (
    ensure_tools, load_devices, save_device, remove_device,
    get_connected_devices, run_cmd, BASE_DIR, CONFIG_FILE
)

app = FastAPI(title="MK DroidScreenCast Web Panel")

STATIC_DIR = BASE_DIR / "static"
TEMPLATES_DIR = BASE_DIR / "templates"
STATIC_DIR.mkdir(exist_ok=True)
TEMPLATES_DIR.mkdir(exist_ok=True)

app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

adb_path = None
scrcpy_path = None



class ConnectionManager:
    
    def __init__(self):
        self.active_connections: List[WebSocket] = []
    
    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.append(websocket)
    
    def disconnect(self, websocket: WebSocket):
        self.active_connections.remove(websocket)
    
    async def broadcast(self, message: dict):
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except:
                pass


manager = ConnectionManager()


@app.on_event("startup")
async def startup_event():
    global adb_path, scrcpy_path
    adb_path, scrcpy_path = ensure_tools()
    
    subprocess.run([str(adb_path), "kill-server"], capture_output=True)
    await asyncio.sleep(1)
    subprocess.run([str(adb_path), "start-server"], capture_output=True)
    
    print(f"✅ ADB: {adb_path}")
    print(f"✅ scrcpy: {scrcpy_path}")
    print(f"🌐 Web Panel: http://localhost:6969")


@app.get("/", response_class=HTMLResponse)
async def get_index():
    html_file = TEMPLATES_DIR / "index.html"
    if html_file.exists():
        return FileResponse(html_file)
    return HTMLResponse("<h1>Создайте файл templates/index.html</h1>")


@app.get("/api/devices")
async def get_devices():
    saved = load_devices().get("devices", [])
    connected = get_connected_devices(adb_path)
    
    return {
        "saved": saved,
        "connected": connected,
        "timestamp": datetime.now().isoformat()
    }


@app.post("/api/connect")
async def connect_device(data: dict):
    address = data.get("address")
    
    if not address:
        raise HTTPException(status_code=400, detail="Address required")
    
    result = subprocess.run(
        [str(adb_path), "connect", address],
        capture_output=True,
        text=True
    )
    
    success = result.returncode == 0 and "connected" in result.stdout.lower()
    
    await manager.broadcast({
        "type": "device_status_changed",
        "address": address,
        "connected": success
    })
    
    return {
        "success": success,
        "output": result.stdout + result.stderr,
        "address": address
    }


@app.post("/api/disconnect")
async def disconnect_device(data: dict):
    address = data.get("address")
    
    result = subprocess.run(
        [str(adb_path), "disconnect", address],
        capture_output=True,
        text=True
    )
    
    await manager.broadcast({
        "type": "device_status_changed",
        "address": address,
        "connected": False
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
        [str(adb_path), "pair", pair_address],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True
    )
    
    output, _ = proc.communicate(input=pair_code + "\n")
    success = "Successfully paired" in output
    
    return {
        "success": success,
        "output": output
    }


@app.post("/api/tcpip")
async def enable_tcpip(data: dict):
    port = data.get("port", "5555")
    
    result = subprocess.run(
        [str(adb_path), "tcpip", port],
        capture_output=True,
        text=True
    )
    
    success = result.returncode == 0
    
    ip_address = None
    if success:
        ip_result = subprocess.run(
            [str(adb_path), "shell", "ip", "addr", "show", "wlan0"],
            capture_output=True,
            text=True
        )
        
        for line in ip_result.stdout.split('\n'):
            if 'inet ' in line and 'inet6' not in line:
                parts = line.strip().split()
                if len(parts) >= 2:
                    ip_address = parts[1].split('/')[0]
                    break
    
    return {
        "success": success,
        "ip": ip_address,
        "port": port,
        "output": result.stdout + result.stderr
    }


@app.post("/api/scrcpy/launch")
async def launch_scrcpy_api(data: dict):
    cmd = [str(scrcpy_path)]
    
    if data.get("bitrate"):
        cmd.extend(["--video-bit-rate", data["bitrate"]])
    if data.get("maxsize"):
        cmd.extend(["--max-size", data["maxsize"]])
    
    keyboard = data.get("keyboard", "uhid")
    cmd.append(f"--keyboard={keyboard}")
    
    if data.get("connection") == "usb":
        cmd.append("--select-usb")
    elif data.get("connection") == "wifi":
        cmd.append("--select-tcpip")
    
    if data.get("stay_awake"):
        cmd.append("--stay-awake")
    if data.get("turn_screen_off"):
        cmd.append("--turn-screen-off")
    if data.get("show_touches"):
        cmd.append("--show-touches")
    if data.get("fullscreen"):
        cmd.append("--fullscreen")
    if data.get("no_audio"):
        cmd.append("--no-audio")
    
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    return {
        "success": True,
        "pid": proc.pid,
        "command": " ".join(cmd)
    }


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await manager.connect(websocket)
    
    try:
        while True:
            devices = get_connected_devices(adb_path)
            await websocket.send_json({
                "type": "devices_update",
                "devices": devices,
                "timestamp": datetime.now().isoformat()
            })
            
            await asyncio.sleep(3)
            
    except WebSocketDisconnect:
        manager.disconnect(websocket)


def run_server(host="0.0.0.0", port=6969):
    uvicorn.run(app, host=host, port=port)


if __name__ == "__main__":
    run_server()
