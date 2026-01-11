import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path

import requests

from .constants import API_LATEST_RELEASE, RELEASES_URL, VERSION
from .paths import BASE_DIR, DOWNLOADS_DIR
from .versioning import fetch_latest_release, is_newer

EXCLUDE_NAMES = {
    ".git",
    ".idea",
    ".venv",
    "__pycache__",
    "BACKUP",
    "config.json",
    "downloads",
    "logs",
}


def _download_file(url, dest):
    with requests.get(url, stream=True, timeout=60) as response:
        response.raise_for_status()
        with open(dest, "wb") as handle:
            for chunk in response.iter_content(chunk_size=8192):
                if chunk:
                    handle.write(chunk)


def _extract_archive(archive_path, dest_dir):
    name = archive_path.name.lower()
    if name.endswith(".zip"):
        with zipfile.ZipFile(archive_path, "r") as zf:
            zf.extractall(dest_dir)
        return
    if name.endswith(".tar.gz") or name.endswith(".tgz"):
        with tarfile.open(archive_path, "r:gz") as tf:
            tf.extractall(dest_dir)
        return
    raise ValueError(f"Unsupported archive format: {archive_path.name}")


def _find_root_dir(extracted_dir):
    entries = [entry for entry in extracted_dir.iterdir() if entry.is_dir()]
    if len(entries) == 1:
        return entries[0]
    return extracted_dir


def _copy_tree(src_dir, dest_dir):
    for item in src_dir.iterdir():
        if item.name in EXCLUDE_NAMES:
            continue
        target = dest_dir / item.name
        if item.is_dir():
            shutil.copytree(item, target, dirs_exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(item, target)


def check_for_updates():
    release = fetch_latest_release(API_LATEST_RELEASE)
    latest = release.get("tag")
    update_available = bool(latest and is_newer(VERSION, latest))
    return {
        "current": VERSION,
        "latest": latest,
        "update_available": update_available,
        "release": release,
    }


def apply_update(release=None, logger=None):
    release = release or fetch_latest_release(API_LATEST_RELEASE)
    latest = release.get("tag")

    if not latest or not is_newer(VERSION, latest):
        return {
            "success": False,
            "message": "Already up to date.",
            "restart_required": False,
            "release_url": release.get("html_url") or RELEASES_URL,
        }

    archive_url = release.get("zipball_url")
    if not archive_url:
        return {
            "success": False,
            "message": "Update archive not available.",
            "restart_required": False,
            "release_url": release.get("html_url") or RELEASES_URL,
        }

    DOWNLOADS_DIR.mkdir(exist_ok=True)
    updates_dir = DOWNLOADS_DIR / "updates"
    updates_dir.mkdir(exist_ok=True)

    if logger:
        logger.info("Downloading update %s", latest)

    with tempfile.TemporaryDirectory(dir=str(updates_dir)) as tmp_dir:
        tmp_path = Path(tmp_dir)
        archive_path = tmp_path / "update.zip"
        _download_file(archive_url, archive_path)

        extract_dir = tmp_path / "extract"
        extract_dir.mkdir()
        _extract_archive(archive_path, extract_dir)

        source_root = _find_root_dir(extract_dir)
        _copy_tree(source_root, BASE_DIR)

    if logger:
        logger.info("Update applied to %s", latest)

    return {
        "success": True,
        "message": f"Updated to {latest}. Restart the app to finish.",
        "restart_required": True,
        "release_url": release.get("html_url") or RELEASES_URL,
    }
