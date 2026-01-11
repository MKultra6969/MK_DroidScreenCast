import json
from copy import deepcopy
from datetime import datetime

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
        "host": "0.0.0.0",
        "port": 6969,
        "auto_open": True,
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
    except Exception:
        return None


def _migrate_config(config):
    changed = False

    if config.get("config_version") != CONFIG_SCHEMA_VERSION:
        config["config_version"] = CONFIG_SCHEMA_VERSION
        changed = True

    presets = config.get("scrcpy", {}).get("presets")
    if not presets:
        config.setdefault("scrcpy", {})["presets"] = deepcopy(DEFAULT_PRESETS)
        changed = True

    if not config.get("devices") and LEGACY_DEVICES_PATH.exists():
        legacy = _load_json(LEGACY_DEVICES_PATH)
        if legacy and isinstance(legacy.get("devices"), list):
            config["devices"] = legacy["devices"]
            changed = True

    if "last_update_check" not in config:
        config["last_update_check"] = None
        changed = True

    return config, changed


def load_config():
    if CONFIG_PATH.exists():
        loaded = _load_json(CONFIG_PATH) or {}
        merged = _deep_merge(DEFAULT_CONFIG, loaded)
        merged, changed = _migrate_config(merged)
        if changed or merged != loaded:
            save_config(merged)
        return merged

    config = deepcopy(DEFAULT_CONFIG)
    config["last_update_check"] = datetime.utcnow().isoformat()
    save_config(config)
    return config


def save_config(config):
    CONFIG_PATH.write_text(
        json.dumps(config, indent=2, ensure_ascii=False),
        encoding="utf-8",
    )


def update_config(patch):
    config = load_config()
    updated = _deep_merge(config, patch)
    save_config(updated)
    return updated
