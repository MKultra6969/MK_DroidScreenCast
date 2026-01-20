"""
Модуль галереи скриншотов.
Позволяет делать скриншоты, хранить их с метаданными и управлять коллекцией.

Эндпоинты:
- GET /api/screenshots - список скриншотов
- POST /api/screenshots/take - сделать скриншот
- GET /api/screenshots/{id} - получить файл скриншота
- PUT /api/screenshots/{id}/caption - обновить подпись
- DELETE /api/screenshots/{id} - удалить скриншот
"""
import json
import math
import subprocess
from datetime import datetime
from typing import Optional, List
from fastapi import APIRouter, HTTPException, Request, Query
from fastapi.responses import FileResponse
from pydantic import BaseModel
from starlette.concurrency import run_in_threadpool
from mkdsc.paths import get_screenshots_dir

router = APIRouter(prefix="/api/screenshots", tags=["screenshots"])

# Директория для скриншотов
def _resolve_paths():
    screenshots_dir = get_screenshots_dir()
    metadata_file = screenshots_dir / "metadata.json"
    return screenshots_dir, metadata_file


class ScreenshotMeta(BaseModel):
    """Метаданные скриншота."""
    id: str
    filename: str
    caption: str = ""
    created_at: str
    size_bytes: int
    device_serial: Optional[str] = None


class CaptionUpdate(BaseModel):
    """Запрос на обновление подписи."""
    caption: str


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


def ensure_dir():
    """Создаёт директорию для скриншотов если её нет."""
    screenshots_dir, metadata_file = _resolve_paths()
    screenshots_dir.mkdir(parents=True, exist_ok=True)
    if not metadata_file.exists():
        metadata_file.write_text("[]", encoding="utf-8")


def load_metadata() -> List[dict]:
    """Загружает метаданные скриншотов."""
    ensure_dir()
    try:
        _, metadata_file = _resolve_paths()
        return json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, Exception):
        return []


def save_metadata(data: List[dict]):
    """Сохраняет метаданные скриншотов."""
    ensure_dir()
    _, metadata_file = _resolve_paths()
    metadata_file.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")


@router.get("")
async def list_screenshots(
    page: int = Query(1, ge=1),
    page_size: int = Query(24, ge=1, le=200)
):
    """Возвращает список всех скриншотов с метаданными."""
    metadata = load_metadata()
    screenshots_dir, _ = _resolve_paths()
    
    # Проверяем существование файлов и обновляем список
    valid_screenshots = []
    for entry in metadata:
        file_path = screenshots_dir / entry.get("filename", "")
        if file_path.exists():
            valid_screenshots.append(entry)
    
    # Сохраняем очищенный список если были удаления
    if len(valid_screenshots) != len(metadata):
        save_metadata(valid_screenshots)
    
    valid_screenshots.sort(key=lambda item: item.get("created_at") or item.get("id") or "", reverse=True)
    total_count = len(valid_screenshots)
    total_pages = max(1, math.ceil(total_count / page_size))
    page = min(page, total_pages)
    start = (page - 1) * page_size
    end = start + page_size
    page_items = valid_screenshots[start:end]

    return {
        "screenshots": page_items,
        "count": total_count,
        "page_count": len(page_items),
        "total_count": total_count,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages
    }


@router.post("/take")
async def take_screenshot(request: Request, serial: Optional[str] = None, caption: str = ""):
    """
    Делает скриншот устройства.
    
    Args:
        serial: Серийный номер устройства (опционально)
        caption: Подпись к скриншоту
    """
    try:
        adb_path = _resolve_adb_path(request)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    ensure_dir()
    
    # Генерируем уникальный ID и имя файла
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    screenshot_id = timestamp
    filename = f"screenshot_{timestamp}.png"
    screenshots_dir, _ = _resolve_paths()
    local_path = screenshots_dir / filename
    device_path = f"/sdcard/screenshot_{timestamp}.png"
    
    # Формируем базовую команду
    cmd_base = [str(adb_path)]
    if serial:
        cmd_base.extend(["-s", serial])
    
    try:
        # Делаем скриншот на устройстве
        result = await run_in_threadpool(
            subprocess.run,
            cmd_base + ["shell", "screencap", "-p", device_path],
            capture_output=True,
            timeout=30
        )
        
        if result.returncode != 0:
            raise HTTPException(
                status_code=500, 
                detail=f"Failed to capture screenshot: {_decode_output(result.stderr)}"
            )
        
        # Копируем на ПК
        result = await run_in_threadpool(
            subprocess.run,
            cmd_base + ["pull", device_path, str(local_path)],
            capture_output=True,
            timeout=30
        )
        
        if result.returncode != 0 or not local_path.exists():
            raise HTTPException(
                status_code=500, 
                detail=f"Failed to pull screenshot: {_decode_output(result.stderr)}"
            )
        
        # Удаляем с устройства
        await run_in_threadpool(
            subprocess.run,
            cmd_base + ["shell", "rm", device_path],
            capture_output=True,
            timeout=10
        )
        
        # Сохраняем метаданные
        metadata = load_metadata()
        entry = {
            "id": screenshot_id,
            "filename": filename,
            "caption": caption,
            "created_at": datetime.now().isoformat(),
            "size_bytes": local_path.stat().st_size,
            "device_serial": serial
        }
        metadata.append(entry)
        save_metadata(metadata)
        
        return {
            "success": True, 
            "screenshot": entry
        }
        
    except subprocess.TimeoutExpired:
        raise HTTPException(status_code=500, detail="Screenshot operation timed out")
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/{screenshot_id}")
async def get_screenshot(screenshot_id: str):
    """Возвращает файл скриншота по ID."""
    metadata = load_metadata()
    entry = next((s for s in metadata if s["id"] == screenshot_id), None)
    
    if not entry:
        raise HTTPException(status_code=404, detail="Screenshot not found")
    
    screenshots_dir, _ = _resolve_paths()
    file_path = screenshots_dir / entry["filename"]
    if not file_path.exists():
        # Удаляем запись о несуществующем файле
        metadata = [s for s in metadata if s["id"] != screenshot_id]
        save_metadata(metadata)
        raise HTTPException(status_code=404, detail="Screenshot file not found")
    
    return FileResponse(
        file_path, 
        media_type="image/png",
        filename=entry["filename"]
    )


@router.put("/{screenshot_id}/caption")
async def update_caption(screenshot_id: str, update: CaptionUpdate):
    """Обновляет подпись скриншота."""
    metadata = load_metadata()
    
    for entry in metadata:
        if entry["id"] == screenshot_id:
            entry["caption"] = update.caption
            save_metadata(metadata)
            return {"success": True, "screenshot": entry}
    
    raise HTTPException(status_code=404, detail="Screenshot not found")


@router.delete("/{screenshot_id}")
async def delete_screenshot(screenshot_id: str):
    """Удаляет скриншот."""
    metadata = load_metadata()
    entry = next((s for s in metadata if s["id"] == screenshot_id), None)
    
    if not entry:
        raise HTTPException(status_code=404, detail="Screenshot not found")
    
    # Удаляем файл
    screenshots_dir, _ = _resolve_paths()
    file_path = screenshots_dir / entry["filename"]
    file_path.unlink(missing_ok=True)
    
    # Удаляем из метаданных
    metadata = [s for s in metadata if s["id"] != screenshot_id]
    save_metadata(metadata)
    
    return {"success": True}


@router.delete("")
async def delete_multiple_screenshots(ids: List[str]):
    """Удаляет несколько скриншотов."""
    metadata = load_metadata()
    screenshots_dir, _ = _resolve_paths()
    deleted = 0
    
    for screenshot_id in ids:
        entry = next((s for s in metadata if s["id"] == screenshot_id), None)
        if entry:
            file_path = screenshots_dir / entry["filename"]
            file_path.unlink(missing_ok=True)
            deleted += 1
    
    # Обновляем метаданные
    metadata = [s for s in metadata if s["id"] not in ids]
    save_metadata(metadata)
    
    return {"success": True, "deleted_count": deleted}
