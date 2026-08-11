import json
import os
import tempfile
import threading
from copy import deepcopy
from datetime import datetime, timezone

from .constants import CONFIG_SCHEMA_VERSION
from .paths import CONFIG_PATH, LEGACY_DEVICES_PATH

DEFAULT_PRESETS = [
    {"name": "FullHD", "bitrate": "8M", "maxsize": "1080"},
    {"name": "2K", "bitrate": "16M", "maxsize": "1440"},
    {"name": "4K", "bitrate": "32M", "maxsize": "2160"},
]

DEFAULT_CONFIG = {
    "config_version": CONFIG_SCHEMA_VERSION,
    "language": "en",
    "web": {
        "host": "127.0.0.1",
        "port": 6969,
        "auto_open": True,
    },
    "logs": {
        "export_dir": "",
    },
    "downloads": {
        "base_dir": "",
    },
    "connection_optimizer": {
        "auto_switch": False,
    },
    "recording": {
        "output_dir": "",
        "format": "mp4",
        "audio_source": "output",
        "file_prefix": "recording",
        "show_preview": True,
        "stay_awake": False,
        "show_touches": False,
        "turn_screen_off": False,
    },
    "cli": {
        "show_banner": True,
    },
    "scrcpy": {
        "bitrate": "8M",
        "maxsize": "1080",
        "keyboard": "uhid",
        "presets": deepcopy(DEFAULT_PRESETS),
        "stay_awake": False,
        "show_touches": False,
        "fullscreen": False,
        "no_audio": False,
        "turn_screen_off": False,
    },
    "devices": [],
    "last_update_check": None,
}

# Sections that must always be objects; anything else is reset to defaults.
_DICT_SECTIONS = (
    "web",
    "logs",
    "downloads",
    "connection_optimizer",
    "recording",
    "cli",
    "scrcpy",
)

# Serialising load/save keeps concurrent endpoints (config, presets, devices)
# from clobbering each other's read-modify-write cycles.
_CONFIG_LOCK = threading.RLock()

# Значения переменной, которые считаем включённым режимом.
_READONLY_VALUES = ("1", "true", "yes", "on")


class ConfigReadOnlyError(RuntimeError):
    """Конфигом владеет другой процесс — запись из этого запрещена."""


def _is_readonly():
    """Режим «только чтение»; включается `MKDSC_CONFIG_READONLY=1`.

    Переменную выставляет Rust-лаунчер при спавне бэкенда. В десктопной сборке
    владелец `config.json` — Rust: он читает, мигрирует и пишет файл. Второй
    писатель файл не повредил бы (запись атомарна), но затирал бы чужие правки
    целиком — каждый процесс сохраняет свою версию, прочитанную до правки
    соседа, и одно из изменений просто исчезает.

    Standalone-режим (`python web_panel.py`, CLI) переменной не видит и
    работает как раньше.
    """
    return os.environ.get("MKDSC_CONFIG_READONLY", "").strip().lower() in _READONLY_VALUES


def _deep_merge(defaults, overrides):
    result = {}
    for key, value in defaults.items():
        if key in overrides:
            override_value = overrides[key]
            if isinstance(value, dict) and isinstance(override_value, dict):
                result[key] = _deep_merge(value, override_value)
            else:
                result[key] = override_value
        else:
            result[key] = value
    for key, value in overrides.items():
        if key not in result:
            result[key] = value
    return result


def _load_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None
    except (OSError, ValueError):
        return None


def _backup_corrupt_config():
    """Keep a copy of an unreadable config instead of silently overwriting it."""
    try:
        if CONFIG_PATH.exists():
            backup = CONFIG_PATH.with_suffix(CONFIG_PATH.suffix + ".bak")
            backup.write_bytes(CONFIG_PATH.read_bytes())
    except OSError:
        pass


def _sanitize_config(config):
    """Force every known section to its expected type.

    A hand-edited (or UI-saved) config with ``"scrcpy": null`` used to make
    every later ``.get()`` raise, which bricked the whole backend.
    """
    changed = False

    if not isinstance(config, dict):
        return deepcopy(DEFAULT_CONFIG), True

    for section in _DICT_SECTIONS:
        if not isinstance(config.get(section), dict):
            config[section] = deepcopy(DEFAULT_CONFIG[section])
            changed = True

    if not isinstance(config.get("devices"), list):
        config["devices"] = []
        changed = True

    if not isinstance(config.get("language"), str):
        config["language"] = DEFAULT_CONFIG["language"]
        changed = True

    presets = config["scrcpy"].get("presets")
    if not isinstance(presets, list):
        config["scrcpy"]["presets"] = deepcopy(DEFAULT_PRESETS)
        changed = True
    else:
        valid = [item for item in presets if isinstance(item, dict) and item.get("name")]
        if len(valid) != len(presets):
            config["scrcpy"]["presets"] = valid
            changed = True

    web = config["web"]
    try:
        port = int(web.get("port", DEFAULT_CONFIG["web"]["port"]))
    except (TypeError, ValueError):
        port = DEFAULT_CONFIG["web"]["port"]
    if not (1 <= port <= 65535):
        port = DEFAULT_CONFIG["web"]["port"]
    if web.get("port") != port:
        web["port"] = port
        changed = True

    if not isinstance(web.get("host"), str) or not web.get("host").strip():
        web["host"] = DEFAULT_CONFIG["web"]["host"]
        changed = True

    return config, changed


def _migrate_config(config):
    config, changed = _sanitize_config(config)

    if config.get("config_version") != CONFIG_SCHEMA_VERSION:
        config["config_version"] = CONFIG_SCHEMA_VERSION
        changed = True

    if not config["scrcpy"].get("presets"):
        config["scrcpy"]["presets"] = deepcopy(DEFAULT_PRESETS)
        changed = True

    if not config.get("devices") and LEGACY_DEVICES_PATH.exists():
        legacy = _load_json(LEGACY_DEVICES_PATH)
        if legacy and isinstance(legacy.get("devices"), list):
            config["devices"] = legacy["devices"]
            changed = True

    if "last_update_check" not in config:
        config["last_update_check"] = None
        changed = True

    if "export_dir" not in config["logs"]:
        config["logs"]["export_dir"] = ""
        changed = True

    if "base_dir" not in config["downloads"]:
        config["downloads"]["base_dir"] = ""
        changed = True

    if "auto_switch" not in config["connection_optimizer"]:
        config["connection_optimizer"]["auto_switch"] = False
        changed = True

    for key, value in DEFAULT_CONFIG["recording"].items():
        if key not in config["recording"]:
            config["recording"][key] = value
            changed = True

    return config, changed


def load_config():
    with _CONFIG_LOCK:
        if CONFIG_PATH.exists():
            loaded = _load_json(CONFIG_PATH)
            if loaded is None:
                _backup_corrupt_config()
                loaded = {}
            elif not isinstance(loaded, dict):
                _backup_corrupt_config()
                loaded = {}
            merged = _deep_merge(DEFAULT_CONFIG, loaded)
            merged, changed = _migrate_config(merged)
            # Самая неочевидная запись во всём модуле: она срабатывает на
            # обычном чтении, без всякого сохранения. В режиме «только чтение»
            # результат миграции остаётся в памяти — закрепит его владелец
            # файла.
            if (changed or merged != loaded) and not _is_readonly():
                save_config(merged)
            return merged

        config = deepcopy(DEFAULT_CONFIG)
        config["last_update_check"] = datetime.now(timezone.utc).isoformat()
        # Первый запуск в десктопе: конфига ещё нет, создаст его Rust. Падать
        # здесь означало бы не поднять бэкенд вовсе.
        if not _is_readonly():
            save_config(config)
        return config


def save_config(config):
    """Write atomically: a crash mid-write must not truncate config.json.

    Бросает ``ConfigReadOnlyError``, если конфигом владеет другой процесс.
    """
    if _is_readonly():
        raise ConfigReadOnlyError(
            "config.json is owned by the desktop app; this process may only read it"
        )
    with _CONFIG_LOCK:
        CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)
        payload = json.dumps(config, indent=2, ensure_ascii=False)

        handle = tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=str(CONFIG_PATH.parent),
            prefix=CONFIG_PATH.name + ".",
            suffix=".tmp",
            delete=False,
        )
        tmp_path = handle.name
        try:
            with handle:
                handle.write(payload)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(tmp_path, CONFIG_PATH)
        except BaseException:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass
            raise


def update_config(patch):
    """Partial update: deep-merge ``patch`` into the stored config."""
    if not isinstance(patch, dict):
        raise ValueError("Config patch must be an object")
    with _CONFIG_LOCK:
        config = load_config()
        updated = _deep_merge(config, patch)
        updated, _ = _migrate_config(updated)
        save_config(updated)
        return updated


def replace_config(new_config):
    """Full replacement: keys absent from ``new_config`` are actually dropped.

    Missing sections fall back to defaults so the app can still boot.
    """
    if not isinstance(new_config, dict):
        raise ValueError("Config payload must be an object")
    with _CONFIG_LOCK:
        merged = _deep_merge(DEFAULT_CONFIG, deepcopy(new_config))
        merged, _ = _migrate_config(merged)
        save_config(merged)
        return merged
