"""
Модуль для оптимизации выбора режима подключения.
Измеряет латентность USB vs WiFi и рекомендует лучшее подключение.

Эндпоинты:
- POST /api/connection/auto-detect - определить лучшее подключение
- GET /api/connection/metrics - получить текущие метрики
"""
import time
import subprocess
from dataclasses import dataclass, asdict
from typing import Optional, List
from fastapi import APIRouter

router = APIRouter(prefix="/api/connection", tags=["connection"])

MAX_WIFI_LATENCY_MS = 120.0
WIFI_BETTER_RATIO = 0.9


@dataclass
class ConnectionMetrics:
    """Метрики подключения устройства."""
    serial: str
    connection_type: str  # 'usb' или 'wifi'
    latency_ms: float
    is_available: bool
    error: Optional[str] = None


def is_usb_connection(serial: str) -> bool:
    """Определяет, является ли подключение USB (не содержит ':' в serial)."""
    return ":" not in serial


def measure_latency(adb_path: str, serial: str, iterations: int = 3) -> float:
    """
    Измеряет латентность команды adb shell echo.
    
    Args:
        adb_path: Путь к adb
        serial: Серийный номер устройства
        iterations: Количество измерений для усреднения
    
    Returns:
        Средняя латентность в миллисекундах
    """
    latencies = []
    
    for _ in range(iterations):
        start = time.perf_counter()
        try:
            result = subprocess.run(
                [adb_path, "-s", serial, "shell", "echo", "ping"],
                capture_output=True, 
                timeout=5
            )
            if result.returncode == 0:
                latency = (time.perf_counter() - start) * 1000
                latencies.append(latency)
        except (subprocess.TimeoutExpired, Exception):
            pass
    
    if not latencies:
        return float('inf')
    
    return sum(latencies) / len(latencies)


def get_connection_metrics(adb_path: str, serial: str) -> ConnectionMetrics:
    """Получает метрики для конкретного подключения."""
    conn_type = "usb" if is_usb_connection(serial) else "wifi"
    
    try:
        latency = measure_latency(adb_path, serial)
        return ConnectionMetrics(
            serial=serial,
            connection_type=conn_type,
            latency_ms=round(latency, 2),
            is_available=latency != float('inf'),
            error=None if latency != float('inf') else "Device not responding"
        )
    except Exception as e:
        return ConnectionMetrics(
            serial=serial,
            connection_type=conn_type,
            latency_ms=float('inf'),
            is_available=False,
            error=str(e)
        )


def _find_wifi_serial(devices: List[dict], ip: Optional[str], port: Optional[str] = None) -> Optional[str]:
    if not ip:
        return None
    for device in devices:
        serial = device.get("serial") or ""
        if not serial.startswith(f"{ip}:"):
            continue
        if port and not serial.endswith(f":{port}"):
            continue
        return serial
    return None


def _run_adb(adb_path: str, args: List[str], timeout: int = 10) -> subprocess.CompletedProcess:
    return subprocess.run([adb_path, *args], capture_output=True, text=True, timeout=timeout)


@router.post("/auto-detect")
async def auto_detect_best_connection():
    """
    Определяет лучшее подключение на основе метрик латентности.
    
    Анализирует все подключенные устройства, измеряет латентность
    и возвращает рекомендацию по выбору подключения.
    """
    from mkdsc.tools import get_connected_devices, get_tool_path
    
    adb_path = get_tool_path("adb")
    devices = get_connected_devices(adb_path)
    
    if not devices:
        return {
            "success": False, 
            "error": "No devices connected",
            "all_metrics": []
        }
    
    metrics: List[ConnectionMetrics] = []
    
    for device in devices:
        serial = device.get("serial")
        if serial:
            metric = get_connection_metrics(adb_path, serial)
            metrics.append(metric)
    
    available_metrics = [m for m in metrics if m.is_available]
    
    if not available_metrics:
        return {
            "success": False,
            "error": "No available devices",
            "all_metrics": [asdict(m) for m in metrics]
        }
    
    # Выбираем подключение с минимальной латентностью
    best = min(available_metrics, key=lambda m: m.latency_ms)
    
    # Если есть USB и WiFi с близкой латентностью - предпочитаем USB
    usb_devices = [m for m in available_metrics if m.connection_type == "usb"]
    if usb_devices:
        best_usb = min(usb_devices, key=lambda m: m.latency_ms)
        # Если USB латентность не сильно хуже (в пределах 20%) - предпочитаем USB
        if best_usb.latency_ms <= best.latency_ms * 1.2:
            best = best_usb
    
    return {
        "success": True,
        "recommended": {
            "serial": best.serial,
            "type": best.connection_type,
            "latency_ms": best.latency_ms
        },
        "all_metrics": [asdict(m) for m in metrics],
        "device_count": {
            "total": len(metrics),
            "usb": len([m for m in metrics if m.connection_type == "usb"]),
            "wifi": len([m for m in metrics if m.connection_type == "wifi"])
        }
    }


@router.post("/auto-switch")
async def auto_switch_best_connection(payload: Optional[dict] = None):
    """
    ???????? ?????? ??????????? ? ???????????? ?? Wi-Fi, ???? ?? ?????? ?????????.
    """
    payload = payload or {}
    target_serial = (payload.get("serial") or "").strip() or None
    port = str(payload.get("port") or "5555").strip() or "5555"

    from mkdsc.tools import get_connected_devices, get_device_wifi_ip, get_tool_path

    adb_path = str(get_tool_path("adb"))
    devices = get_connected_devices(adb_path)

    if not devices:
        return {"success": False, "error": "No devices connected"}

    if target_serial and not any(d.get("serial") == target_serial for d in devices):
        return {"success": False, "error": "Device not connected"}

    if not target_serial:
        usb_devices = [d.get("serial") for d in devices if d.get("serial") and is_usb_connection(d["serial"])]
        target_serial = usb_devices[0] if usb_devices else devices[0].get("serial")

    if not target_serial:
        return {"success": False, "error": "No valid device serial found"}

    usb_serial = target_serial if is_usb_connection(target_serial) else None
    wifi_serial = target_serial if not is_usb_connection(target_serial) else None
    attempted_tcpip = False

    if usb_serial:
        ip_address = get_device_wifi_ip(adb_path, usb_serial)
        wifi_serial = _find_wifi_serial(devices, ip_address, port)
        if ip_address and not wifi_serial:
            attempted_tcpip = True
            try:
                _run_adb(adb_path, ["-s", usb_serial, "tcpip", port], timeout=10)
                _run_adb(adb_path, ["connect", f"{ip_address}:{port}"], timeout=10)
            except Exception:
                pass
            devices = get_connected_devices(adb_path)
            wifi_serial = _find_wifi_serial(devices, ip_address, port)

    usb_metric = get_connection_metrics(adb_path, usb_serial) if usb_serial else None
    wifi_metric = get_connection_metrics(adb_path, wifi_serial) if wifi_serial else None

    recommended = None
    if wifi_metric and wifi_metric.is_available:
        wifi_fast = (
            wifi_metric.latency_ms <= MAX_WIFI_LATENCY_MS
            and (not usb_metric or not usb_metric.is_available or wifi_metric.latency_ms < usb_metric.latency_ms * WIFI_BETTER_RATIO)
        )
        if wifi_fast:
            recommended = wifi_metric

    if not recommended and usb_metric and usb_metric.is_available:
        recommended = usb_metric
    if not recommended and wifi_metric and wifi_metric.is_available:
        recommended = wifi_metric

    if not recommended:
        return {"success": False, "error": "No responsive devices"}

    return {
        "success": True,
        "recommended": {
            "serial": recommended.serial,
            "type": recommended.connection_type,
            "latency_ms": recommended.latency_ms,
        },
        "metrics": {
            "usb": asdict(usb_metric) if usb_metric else None,
            "wifi": asdict(wifi_metric) if wifi_metric else None,
        },
        "attempted_tcpip": attempted_tcpip,
    }


@router.get("/metrics/{serial}")
async def get_device_metrics(serial: str):
    """Получает метрики для конкретного устройства."""
    from mkdsc.tools import get_tool_path
    
    adb_path = get_tool_path("adb")
    metric = get_connection_metrics(adb_path, serial)
    
    return asdict(metric)


@router.get("/metrics")
async def get_all_metrics():
    """Получает метрики для всех подключенных устройств."""
    from mkdsc.tools import get_connected_devices, get_tool_path
    
    adb_path = get_tool_path("adb")
    devices = get_connected_devices(adb_path)
    
    metrics = []
    for device in devices:
        serial = device.get("serial")
        if serial:
            metric = get_connection_metrics(adb_path, serial)
            metrics.append(asdict(metric))
    
    return {
        "metrics": metrics,
        "device_count": len(metrics)
    }
