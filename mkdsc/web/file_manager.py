"""
Файловый менеджер для работы с файлами на Android устройстве.
Поддерживает листинг, загрузку, скачивание и удаление файлов.

Эндпоинты:
- GET /api/files/list - листинг директории
- POST /api/files/upload - загрузка файла на устройство (adb push)
- GET /api/files/download - скачивание файла (adb pull)
- DELETE /api/files/delete - удаление файла
- POST /api/files/mkdir - создание директории
"""
import math
import posixpath
import shlex
import subprocess
import tempfile
import shutil
from pathlib import Path
from typing import Optional, List
from fastapi import APIRouter, UploadFile, File, HTTPException, Query, Request
from fastapi.responses import FileResponse
from pydantic import BaseModel
from starlette.background import BackgroundTask
from starlette.concurrency import run_in_threadpool
from mkdsc.paths import DATA_DIR, get_downloads_base_dir

router = APIRouter(prefix="/api/files", tags=["files"])

UPLOAD_CHUNK_SIZE = 1024 * 1024


class FileInfo(BaseModel):
    """Информация о файле/директории."""
    name: str
    path: str
    is_dir: bool
    size: int = 0
    permissions: str = ""
    date: str = ""
    is_link: bool = False
    link_target: str = ""


class DeleteRequest(BaseModel):
    """Запрос на удаление файла."""
    path: str


class MkdirRequest(BaseModel):
    """Запрос на создание директории."""
    path: str


class MoveRequest(BaseModel):
    source: str
    destination: str


class WriteRequest(BaseModel):
    path: str
    content: str


class PullRequest(BaseModel):
    path: str
    destination_dir: Optional[str] = None


# Точные пути, которые нельзя удалять/перемещать/перезаписывать.
_PROTECTED_ROOTS = frozenset({
    "/",
    "/system",
    "/data",
    "/vendor",
    "/sdcard",
    "/storage",
    "/storage/emulated",
    "/storage/emulated/0",
    "/storage/self",
    "/storage/self/primary",
    "/mnt",
    "/mnt/sdcard",
})

# Целые поддеревья, куда запись/удаление запрещены.
_PROTECTED_PREFIXES = ("/system", "/vendor", "/proc", "/sys", "/dev", "/apex", "/boot")


def _resolve_adb_path(request: Request):
    adb_path = getattr(request.app.state, "adb_path", None)
    if adb_path:
        return adb_path
    from mkdsc.tools import get_tool_path
    return get_tool_path("adb")


def _decode_output(raw: Optional[bytes]) -> str:
    if not raw:
        return ""
    return raw.decode("utf-8", errors="replace")


def _resolve_download_dir(request: Request, destination_dir: Optional[str]) -> Path:
    if destination_dir:
        base_dir = Path(destination_dir).expanduser()
    else:
        # Читаем конфиг заново: app.state.config замораживается на старте и
        # смена папки загрузок в UI до перезапуска не применялась.
        from mkdsc.config import load_config
        base_dir = get_downloads_base_dir(load_config())
        if base_dir == DATA_DIR:
            base_dir = DATA_DIR / "downloads"

    base_dir.mkdir(parents=True, exist_ok=True)
    return base_dir


def _normalize_path(path: str) -> str:
    """Схлопывает '.', '..' и повторные слэши, чтобы проверки нельзя было обойти."""
    if not path:
        return "/"
    candidate = path.strip().replace("\\", "/")
    if not candidate.startswith("/"):
        candidate = "/" + candidate
    normalized = posixpath.normpath(candidate)
    while normalized.startswith("//"):
        normalized = normalized[1:]
    return normalized or "/"


def _is_protected_path(path: str) -> bool:
    normalized = _normalize_path(path)
    if normalized in _PROTECTED_ROOTS:
        return True
    for prefix in _PROTECTED_PREFIXES:
        if normalized == prefix or normalized.startswith(prefix + "/"):
            return True
    return False


def _is_dangerous_root(path: str) -> bool:
    return _is_protected_path(path)


def _safe_filename(name: Optional[str]) -> str:
    """Оставляет только базовое имя: '../../evil' не должен уйти за пределы папки."""
    candidate = (name or "").replace("\\", "/").strip()
    candidate = candidate.rsplit("/", 1)[-1].strip()
    if not candidate or candidate in (".", "..") or "\x00" in candidate:
        raise HTTPException(status_code=400, detail="Invalid file name")
    return candidate


def _join_device_path(directory: str, name: str) -> str:
    return f"{_normalize_path(directory).rstrip('/')}/{name}"


def _shell_string(command: str | List[str]) -> str:
    """adb shell склеивает argv в одну строку, поэтому квотируем сами.

    Без этого 'rm -rf /sdcard/My Folder' на устройстве превращается в удаление
    двух разных путей.
    """
    if isinstance(command, (list, tuple)):
        return " ".join(shlex.quote(str(arg)) for arg in command)
    return command


async def _run_adb_shell(
    adb_path: Path | str,
    serial: Optional[str],
    command: str | List[str],
    timeout: int = 30
) -> subprocess.CompletedProcess:
    cmd = [str(adb_path)]
    if serial:
        cmd.extend(["-s", serial])
    cmd.extend(["shell", _shell_string(command)])
    return await run_in_threadpool(subprocess.run, cmd, capture_output=True, timeout=timeout)


def _list_target(path: str) -> str:
    """Путь для ``ls`` — всегда с завершающей косой чертой.

    Без неё ``ls -la /sdcard`` описывает саму ссылку, а не то, куда она ведёт:
    на Android ``/sdcard`` — симлинк на ``/storage/self/primary``, и файловый
    менеджер открывался на списке из одной строки вместо содержимого папки.

    Пути дочерних записей строятся из исходного ``path``, иначе они получили бы
    адреса вида ``/sdcard//DCIM``.
    """
    return f"{path.rstrip('/')}/"


def parse_ls_output(output: str, base_path: str) -> List[FileInfo]:
    """
    Парсит вывод команды ls -la.

    Args:
        output: Вывод ls -la
        base_path: Базовый путь директории

    Returns:
        Список FileInfo объектов
    """
    files = []
    lines = output.strip().split("\n")

    for line in lines:
        # Пропускаем "total X" и пустые строки
        if not line or line.startswith("total ") or line.startswith("ls:"):
            continue

        parts = line.split()
        if len(parts) < 7:
            continue

        permissions = parts[0]
        name = " ".join(parts[7:]) if len(parts) > 7 else parts[-1]

        # Симлинки печатаются как "name -> target": цель в имя не входит.
        is_link = permissions.startswith("l")
        link_target = ""
        if is_link and " -> " in name:
            name, link_target = name.split(" -> ", 1)
            name = name.strip()
            link_target = link_target.strip()

        # Пропускаем . и ..
        if name in (".", ".."):
            continue

        try:
            size = int(parts[4]) if parts[4].isdigit() else 0
        except (ValueError, IndexError):
            size = 0

        date = f"{parts[5]} {parts[6]}" if len(parts) >= 7 else ""
        is_dir = permissions.startswith("d")

        full_path = f"{base_path.rstrip('/')}/{name}"

        files.append(FileInfo(
            name=name,
            path=full_path,
            is_dir=is_dir,
            size=size,
            permissions=permissions,
            date=date,
            is_link=is_link,
            link_target=link_target,
        ))

    return files


def parse_simple_ls_output(output: str, base_path: str) -> List[FileInfo]:
    """
    Fallback parser for plain ls output (one entry per line).
    """
    files = []
    lines = output.strip().split("\n")

    for line in lines:
        name = line.strip()
        if not name or name in (".", ".."):
            continue
        if name.startswith("ls:"):
            continue

        is_dir = name.endswith("/")
        if is_dir:
            name = name[:-1]

        full_path = f"{base_path.rstrip('/')}/{name}"
        files.append(FileInfo(
            name=name,
            path=full_path,
            is_dir=is_dir,
            size=0,
            permissions="",
            date=""
        ))

    return files


async def _resolve_link_dirs(adb_path, serial: Optional[str], files: List[FileInfo]) -> None:
    """Одним вызовом определяет, какие симлинки указывают на директории.

    На Android /sdcard — симлинк, и без этого он не открывался бы в UI.
    """
    links = [item for item in files if item.is_link]
    if not links:
        return

    script = "; ".join(
        f'if [ -d {shlex.quote(item.path)} ]; then echo {shlex.quote(item.path)}; fi'
        for item in links
    )
    try:
        result = await _run_adb_shell(adb_path, serial, script, timeout=15)
    except Exception:
        return

    dir_paths = {line.strip() for line in _decode_output(result.stdout).splitlines() if line.strip()}
    for item in links:
        if item.path in dir_paths:
            item.is_dir = True


@router.get("/list")
async def list_directory(
    request: Request,
    path: str = Query("/sdcard", description="Path to list"),
    serial: Optional[str] = Query(None, description="Device serial"),
    page: int = Query(1, ge=1),
    page_size: int = Query(100, ge=1, le=500)
):
    """
    Получает содержимое директории на устройстве.

    Args:
        path: Путь к директории (по умолчанию /sdcard)
        serial: Серийный номер устройства
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    logger = getattr(request.app.state, "logger", None)

    try:
        target = _list_target(path)
        result = await _run_adb_shell(adb_path, serial, ["ls", "-la", target], timeout=30)
        if logger:
            logger.info("files.list path=%s serial=%s code=%s", path, serial or "-", result.returncode)

        stderr = _decode_output(result.stderr).strip()
        stdout_text = _decode_output(result.stdout)
        ls_error = stderr
        if not ls_error:
            for line in stdout_text.splitlines():
                if line.startswith("ls:"):
                    ls_error = line.strip()
                    break
        if not ls_error:
            if "Permission denied" in stdout_text:
                ls_error = "Permission denied"
            elif "No such file" in stdout_text or "not a directory" in stdout_text:
                ls_error = "Path not found"

        if result.returncode != 0 or ls_error:
            # Отдаём осмысленный код, а не общий 500
            if logger and ls_error:
                logger.info("files.list error=%s", ls_error)
            if ls_error and ("No such file" in ls_error or "not a directory" in ls_error):
                raise HTTPException(status_code=404, detail=ls_error)
            if ls_error and "Permission denied" in ls_error:
                raise HTTPException(status_code=403, detail=ls_error)
            if result.returncode != 0:
                raise HTTPException(status_code=400, detail=ls_error or stderr or "ls failed")

        files = parse_ls_output(stdout_text, path)
        if not files and stdout_text.strip():
            fallback = await _run_adb_shell(adb_path, serial, ["ls", "-p", target], timeout=30)
            fallback_text = _decode_output(fallback.stdout)
            files = parse_simple_ls_output(fallback_text, path)
            if logger:
                logger.info("files.list fallback=%s path=%s raw=%s", len(files), path, len(fallback_text))

        await _resolve_link_dirs(adb_path, serial, files)

        # Сортируем: сначала директории, потом файлы
        files.sort(key=lambda f: (not f.is_dir, f.name.lower()))
        total_count = len(files)
        total_pages = max(1, math.ceil(total_count / page_size))
        page = min(page, total_pages)
        start = (page - 1) * page_size
        end = start + page_size
        files_page = files[start:end]
        if logger:
            logger.info("files.list count=%s path=%s raw=%s", len(files), path, len(stdout_text))

        return {
            "path": path,
            "files": [f.model_dump() for f in files_page],
            "count": len(files_page),
            "total_count": total_count,
            "page": page,
            "page_size": page_size,
            "total_pages": total_pages
        }

    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/upload")
async def upload_file(
    request: Request,
    file: UploadFile = File(...),
    destination: str = Query("/sdcard", description="Destination path on device"),
    serial: Optional[str] = Query(None, description="Device serial")
):
    """
    Загружает файл на устройство (adb push).

    Args:
        file: Файл для загрузки
        destination: Путь на устройстве
        serial: Серийный номер устройства
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    # Имя приходит из multipart и полностью подконтрольно клиенту.
    filename = _safe_filename(file.filename)
    dest_dir = _normalize_path(destination)
    if _is_protected_path(dest_dir):
        raise HTTPException(status_code=400, detail="Cannot upload to system directories")

    tmp_dir = Path(tempfile.mkdtemp())
    tmp_path = tmp_dir / filename

    try:
        # Пишем потоком: файл на несколько гигабайт не должен оказаться в RAM.
        size = 0
        with tmp_path.open("wb") as handle:
            while True:
                chunk = await file.read(UPLOAD_CHUNK_SIZE)
                if not chunk:
                    break
                size += len(chunk)
                await run_in_threadpool(handle.write, chunk)

        dest_path = _join_device_path(dest_dir, filename)

        cmd = [str(adb_path)]
        if serial:
            cmd.extend(["-s", serial])
        cmd.extend(["push", str(tmp_path), dest_path])

        result = await run_in_threadpool(subprocess.run, cmd, capture_output=True, timeout=600)

        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=_decode_output(result.stderr))

        return {
            "success": True,
            "path": dest_path,
            "filename": filename,
            "size": size
        }

    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Upload timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        # Очищаем временные файлы
        shutil.rmtree(tmp_dir, ignore_errors=True)


@router.get("/download")
async def download_file(
    request: Request,
    path: str = Query(..., description="File path on device"),
    serial: Optional[str] = Query(None, description="Device serial")
):
    """
    Скачивает файл с устройства (adb pull).

    Args:
        path: Путь к файлу на устройстве
        serial: Серийный номер устройства
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    filename = _safe_filename(Path(_normalize_path(path)).name)
    tmp_dir = Path(tempfile.mkdtemp())
    tmp_path = tmp_dir / filename

    try:
        cmd = [str(adb_path)]
        if serial:
            cmd.extend(["-s", serial])
        cmd.extend(["pull", path, str(tmp_path)])

        result = await run_in_threadpool(subprocess.run, cmd, capture_output=True, timeout=600)

        if result.returncode != 0 or not tmp_path.exists():
            stderr = _decode_output(result.stderr)
            raise HTTPException(status_code=400, detail=stderr or "File not found")

        # Без background-задачи копия оставалась в %TEMP% навсегда.
        return FileResponse(
            tmp_path,
            filename=filename,
            media_type="application/octet-stream",
            background=BackgroundTask(shutil.rmtree, tmp_dir, ignore_errors=True),
        )

    except subprocess.TimeoutExpired:
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise HTTPException(status_code=500, detail="Download timed out")
    except HTTPException:
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise
    except Exception as e:
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise HTTPException(status_code=500, detail=str(e))


@router.delete("/delete")
async def delete_file(
    request: Request,
    path: str = Query(..., description="File/directory path to delete"),
    serial: Optional[str] = Query(None, description="Device serial")
):
    """
    Удаляет файл или директорию на устройстве.

    Args:
        path: Путь к файлу/директории
        serial: Серийный номер устройства
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    # Проверка безопасности - не удаляем системные директории
    if _is_protected_path(path):
        raise HTTPException(status_code=400, detail="Cannot delete system directories")

    try:
        result = await _run_adb_shell(adb_path, serial, ["rm", "-rf", path], timeout=30)
        stderr = _decode_output(result.stderr).strip()
        stdout = _decode_output(result.stdout).strip()
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=stderr or stdout or "Delete failed")

        return {
            "success": True,
            "path": path
        }

    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Delete operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/mkdir")
async def make_directory(payload: MkdirRequest, request: Request, serial: Optional[str] = None):
    """
    Создаёт директорию на устройстве.

    Args:
        request: Запрос с путём директории
        serial: Серийный номер устройства
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    logger = getattr(request.app.state, "logger", None)
    try:
        result = await _run_adb_shell(adb_path, serial, ["mkdir", "-p", payload.path], timeout=30)
        if logger:
            logger.info("files.mkdir path=%s serial=%s code=%s", payload.path, serial or "-", result.returncode)
        stderr = _decode_output(result.stderr).strip()
        stdout = _decode_output(result.stdout).strip()
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=stderr or stdout or "Mkdir failed")

        return {
            "success": True,
            "path": payload.path
        }

    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Mkdir operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/move")
async def move_path(payload: MoveRequest, request: Request, serial: Optional[str] = None):
    """
    Перемещает/переименовывает файл или директорию.
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    if _is_protected_path(payload.source) or _is_protected_path(payload.destination):
        raise HTTPException(status_code=400, detail="Cannot move system directories")

    try:
        result = await _run_adb_shell(
            adb_path,
            serial,
            ["mv", payload.source, payload.destination],
            timeout=60
        )
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=_decode_output(result.stderr))
        return {"success": True, "path": payload.destination}
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Move operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/read")
async def read_file(
    request: Request,
    path: str = Query(..., description="File path on device"),
    max_bytes: int = Query(262144, description="Max bytes to read"),
    serial: Optional[str] = Query(None, description="Device serial")
):
    """
    Читает начало файла (с ограничением по размеру).
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    if max_bytes <= 0:
        raise HTTPException(status_code=400, detail="max_bytes must be positive")
    max_bytes = min(max_bytes, 1024 * 1024)

    try:
        # head -c обрезает на устройстве; cat тянул файл целиком в память
        # и только потом отрезал первые 256 КБ.
        result = await _run_adb_shell(
            adb_path, serial, ["head", "-c", str(max_bytes + 1), path], timeout=30
        )
        if result.returncode != 0:
            # Старые/урезанные прошивки могут не иметь head -c.
            fallback = await _run_adb_shell(adb_path, serial, ["cat", path], timeout=30)
            if fallback.returncode != 0:
                raise HTTPException(status_code=400, detail=_decode_output(result.stderr))
            result = fallback

        raw = result.stdout or b""
        truncated = len(raw) > max_bytes
        if truncated:
            raw = raw[:max_bytes]
        is_binary = b"\x00" in raw
        content = raw.decode("utf-8", errors="replace")
        return {
            "path": path,
            "content": content,
            "truncated": truncated,
            "is_binary": is_binary
        }
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Read operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/write")
async def write_file(payload: WriteRequest, request: Request, serial: Optional[str] = None):
    """
    Пишет текстовый файл на устройство (adb push).
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    if _is_protected_path(payload.path):
        raise HTTPException(status_code=400, detail="Cannot write to system directories")

    tmp_dir = Path(tempfile.mkdtemp())
    tmp_path = tmp_dir / _safe_filename(Path(_normalize_path(payload.path)).name)

    try:
        tmp_path.write_text(payload.content, encoding="utf-8")

        cmd = [str(adb_path)]
        if serial:
            cmd.extend(["-s", serial])
        cmd.extend(["push", str(tmp_path), payload.path])

        result = await run_in_threadpool(subprocess.run, cmd, capture_output=True, timeout=120)
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=_decode_output(result.stderr))

        return {
            "success": True,
            "path": payload.path,
            "size": len(payload.content.encode("utf-8"))
        }
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Write operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)


@router.post("/pull")
async def pull_file(payload: PullRequest, request: Request, serial: Optional[str] = None):
    """
    Скачивает файл с устройства в локальную папку загрузок (adb pull).
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))

    logger = getattr(request.app.state, "logger", None)
    target_dir = _resolve_download_dir(request, payload.destination_dir)
    source_path = payload.path

    try:
        cmd = [str(adb_path)]
        if serial:
            cmd.extend(["-s", serial])
        cmd.extend(["pull", source_path, str(target_dir)])

        result = await run_in_threadpool(subprocess.run, cmd, capture_output=True, timeout=600)
        stderr = _decode_output(result.stderr).strip()
        stdout = _decode_output(result.stdout).strip()
        if logger:
            logger.info("files.pull path=%s serial=%s code=%s", source_path, serial or "-", result.returncode)
        if result.returncode != 0:
            raise HTTPException(status_code=400, detail=stderr or stdout or "Pull failed")

        final_name = Path(source_path.rstrip("/")).name
        final_path = target_dir / final_name

        return {
            "success": True,
            "path": str(final_path)
        }
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Pull operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
