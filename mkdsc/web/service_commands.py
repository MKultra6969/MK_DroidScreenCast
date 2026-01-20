"""
Модуль для выполнения сервисных ADB команд.
Предоставляет быстрый доступ к диагностике устройства.

Эндпоинты:
- GET /api/service/{command_name} - выполнить предопределённую команду
- POST /api/service/custom - выполнить кастомную ADB команду
"""
import shlex
import subprocess
from fastapi import APIRouter, HTTPException, Request
from starlette.concurrency import run_in_threadpool
from typing import Optional, Sequence, Union
from pydantic import BaseModel

router = APIRouter(prefix="/api/service", tags=["service"])
DEFAULT_TIMEOUT_SECONDS = 30

# Предопределённые команды для быстрого доступа
PREDEFINED_COMMANDS = {
    "battery": ("dumpsys battery", 80),
    "wifi": ("cmd wifi status", 80),
    "top": ("top -n 1 -b", 50),
    "props": ("getprop", 100),
    "memory": ("cat /proc/meminfo", 80),
    "cpu": ("cat /proc/cpuinfo", 50),
    "disk": ("df -h", None),
    "packages": ("pm list packages", 100),
    "screen": (["wm size", "wm density"], None),
    "processes": ("ps -A", 50),
    "uptime": ("uptime", 20),
    "network": ("ip addr show", 120),
    "thermal": ("dumpsys thermalservice", 120),
}

CommandSpec = Union[str, Sequence[str]]
SHELL_META_CHARS = set("|&;<>()$`\\\n")


class CommandRequest(BaseModel):
    """Запрос на выполнение кастомной команды."""
    command: str
    serial: Optional[str] = None


class CommandResponse(BaseModel):
    """Ответ с результатом выполнения команды."""
    success: bool
    output: str
    error: Optional[str] = None
    command: str


def _resolve_adb_path(request: Request):
    adb_path = getattr(request.app.state, "adb_path", None)
    if adb_path:
        return adb_path
    from mkdsc.tools import get_tool_path
    return get_tool_path("adb")


def _trim_output(output: str, max_lines: Optional[int]) -> str:
    if not output or not max_lines:
        return output
    lines = output.splitlines()
    if len(lines) <= max_lines:
        return output
    trimmed = "\n".join(lines[:max_lines])
    return f"{trimmed}\n... ({len(lines) - max_lines} more lines truncated)"


def _decode_output(raw: Optional[bytes]) -> str:
    if not raw:
        return ""
    return raw.decode("utf-8", errors="replace")


def _needs_shell(command: str) -> bool:
    return any(ch in command for ch in SHELL_META_CHARS)


def _build_adb_command(adb_path: str, serial: Optional[str], command: str) -> list[str]:
    cmd = [str(adb_path)]
    if serial:
        cmd.extend(["-s", serial])
    if _needs_shell(command):
        cmd.extend(["shell", "sh", "-c", command])
    else:
        cmd.extend(["shell", *shlex.split(command)])
    return cmd


def _run_adb_command_single(
    adb_path: str,
    command: str,
    serial: Optional[str] = None,
    max_lines: Optional[int] = None,
    timeout_seconds: int = DEFAULT_TIMEOUT_SECONDS
) -> CommandResponse:
    cmd = _build_adb_command(adb_path, serial, command)
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            timeout=timeout_seconds
        )
        stdout = _decode_output(result.stdout)
        stderr = _decode_output(result.stderr)
        output = stdout if stdout else stderr
        output = _trim_output(output, max_lines)
        return CommandResponse(
            success=result.returncode == 0,
            output=output,
            error=stderr if result.returncode != 0 else None,
            command=" ".join(cmd)
        )
    except subprocess.TimeoutExpired as exc:
        stdout = _decode_output(exc.stdout)
        stderr = _decode_output(exc.stderr)
        output = stdout if stdout else stderr
        output = _trim_output(output, max_lines)
        if output:
            output = f"{output}\n\n[Timed out after {timeout_seconds} seconds]"
        return CommandResponse(
            success=False,
            output=output,
            error=f"Command timed out after {timeout_seconds} seconds",
            command=" ".join(cmd)
        )
    except Exception as e:
        return CommandResponse(
            success=False,
            output="",
            error=str(e),
            command=" ".join(cmd)
        )


def run_adb_command(
    adb_path: str,
    command: CommandSpec,
    serial: Optional[str] = None,
    max_lines: Optional[int] = None,
    timeout_seconds: int = DEFAULT_TIMEOUT_SECONDS
) -> CommandResponse:
    """
    Выполняет ADB команду и возвращает результат.
    
    Args:
        adb_path: Путь к исполняемому файлу adb
        command: Команда для выполнения (без префикса 'adb')
        serial: Серийный номер устройства (опционально)
    
    Returns:
        CommandResponse с результатом выполнения
    """
    commands = list(command) if isinstance(command, (list, tuple)) else [command]
    combined_output = []
    combined_errors = []
    success = True
    executed = []

    for item in commands:
        result = _run_adb_command_single(adb_path, item, serial, max_lines, timeout_seconds)
        executed.append(result.command)
        if result.output:
            combined_output.append(result.output)
        if not result.success:
            success = False
            if result.error:
                combined_errors.append(result.error)

    return CommandResponse(
        success=success,
        output="\n\n".join(combined_output),
        error="\n".join(combined_errors) if combined_errors else None,
        command=" ; ".join(executed)
    )


@router.get("/commands")
async def list_available_commands():
    """Возвращает список доступных предопределённых команд."""
    return {
        "commands": list(PREDEFINED_COMMANDS.keys()),
        "descriptions": {
            "battery": "Battery status and health",
            "wifi": "WiFi connection info",
            "top": "Running processes (top)",
            "props": "System properties",
            "memory": "Memory info (/proc/meminfo)",
            "cpu": "CPU info (/proc/cpuinfo)",
            "disk": "Disk usage (df -h)",
            "packages": "Installed packages",
            "screen": "Screen size and density",
            "processes": "Process list (ps)",
            "uptime": "Device uptime",
            "network": "Network interfaces",
            "thermal": "Thermal service status",
        }
    }


@router.get("/{command_name}")
async def run_predefined_command(command_name: str, request: Request, serial: Optional[str] = None):
    """
    Выполняет предопределённую ADB команду.
    
    Args:
        command_name: Имя команды из списка PREDEFINED_COMMANDS
        serial: Серийный номер устройства (опционально)
    """
    if command_name not in PREDEFINED_COMMANDS:
        raise HTTPException(
            status_code=404, 
            detail=f"Unknown command: {command_name}. Available: {list(PREDEFINED_COMMANDS.keys())}"
        )
    
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    command, max_lines = PREDEFINED_COMMANDS[command_name]

    return await run_in_threadpool(run_adb_command, adb_path, command, serial, max_lines, DEFAULT_TIMEOUT_SECONDS)


@router.post("/custom")
async def run_custom_command(request: CommandRequest, http_request: Request):
    """
    Выполняет кастомную ADB команду.
    
    ВНИМАНИЕ: Опасные команды блокируются для безопасности.
    """
    # Базовая валидация безопасности
    dangerous_patterns = [
        "rm -rf /",
        "rm -rf /*",
        "format",
        "wipe",
        "factory",
        "reboot bootloader",
        "reboot recovery",
        "flash",
    ]
    
    command_lower = request.command.lower()
    for pattern in dangerous_patterns:
        if pattern in command_lower:
            raise HTTPException(
                status_code=400, 
                detail=f"Dangerous command blocked: contains '{pattern}'"
            )
    
    try:
        adb_path = _resolve_adb_path(http_request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    
    return await run_in_threadpool(
        run_adb_command,
        adb_path,
        request.command,
        request.serial,
        None,
        DEFAULT_TIMEOUT_SECONDS
    )
