import os
from pathlib import Path

_base_dir_env = os.environ.get("MKDSC_BASE_DIR")
BASE_DIR = Path(_base_dir_env) if _base_dir_env else Path(__file__).resolve().parent.parent

_data_dir_env = os.environ.get("MKDSC_DATA_DIR")
DATA_DIR = Path(_data_dir_env) if _data_dir_env else BASE_DIR

DOWNLOADS_DIR = DATA_DIR / "downloads"
LOGS_DIR = DATA_DIR / "logs"
RECORDINGS_DIR = DATA_DIR / "recordings"
STATIC_DIR = BASE_DIR / "static"
TEMPLATES_DIR = BASE_DIR / "templates"
CONFIG_PATH = DATA_DIR / "config.json"
LEGACY_DEVICES_PATH = BASE_DIR / "devices.json"


def get_downloads_base_dir(config=None) -> Path:
    if config is None:
        from .config import load_config
        config = load_config()

    base_dir = ""
    if isinstance(config, dict):
        downloads_cfg = config.get("downloads")
        if isinstance(downloads_cfg, dict):
            base_dir = str(downloads_cfg.get("base_dir") or "").strip()

    return Path(base_dir).expanduser() if base_dir else DATA_DIR


def get_logs_dir(config=None) -> Path:
    base_dir = get_downloads_base_dir(config)
    if base_dir == DATA_DIR:
        return LOGS_DIR
    return base_dir / "logs"


def get_recordings_dir(config=None) -> Path:
    base_dir = get_downloads_base_dir(config)
    if base_dir == DATA_DIR:
        return RECORDINGS_DIR
    return base_dir / "video"


def get_screenshots_dir(config=None) -> Path:
    base_dir = get_downloads_base_dir(config)
    if base_dir == DATA_DIR:
        return DATA_DIR / "screenshots"
    return base_dir / "screenshots"
