import copy
import json

import pytest

from mkdsc import config as config_module
from mkdsc.config import (
    DEFAULT_CONFIG,
    _deep_merge,
    _migrate_config,
    load_config,
    replace_config,
    save_config,
    update_config,
)


@pytest.fixture(autouse=True)
def isolated_config(tmp_path, monkeypatch):
    path = tmp_path / "config.json"
    monkeypatch.setattr(config_module, "CONFIG_PATH", path)
    return path


def test_default_host_is_loopback():
    assert DEFAULT_CONFIG["web"]["host"] == "127.0.0.1"


@pytest.mark.parametrize(
    "payload",
    [
        {"scrcpy": None},
        {"web": 5},
        {"logs": []},
        {"recording": "x"},
        {"devices": 3},
        {"connection_optimizer": None},
    ],
)
def test_migrate_survives_wrong_types(payload):
    """BUG-09: a single null section used to make every endpoint 500 forever."""
    merged = _deep_merge(copy.deepcopy(DEFAULT_CONFIG), payload)
    result, _ = _migrate_config(merged)
    assert isinstance(result["scrcpy"], dict)
    assert isinstance(result["web"], dict)
    assert isinstance(result["devices"], list)


def test_load_config_recovers_from_null_section(isolated_config):
    save_config({**copy.deepcopy(DEFAULT_CONFIG), "scrcpy": None})
    loaded = load_config()
    assert isinstance(loaded["scrcpy"], dict)
    assert loaded["scrcpy"]["presets"]


def test_load_config_backs_up_corrupt_file(isolated_config):
    isolated_config.write_text("{not json", encoding="utf-8")
    loaded = load_config()
    assert loaded["web"]["port"] == 6969
    assert isolated_config.with_suffix(".json.bak").exists()


def test_update_config_merges_but_cannot_delete(isolated_config):
    """BUG-10: POST is a patch; it must not be used for deletions."""
    update_config({"scrcpy": {"custom": "x"}})
    update_config({"scrcpy": {}})
    assert load_config()["scrcpy"]["custom"] == "x"


def test_replace_config_deletes_removed_keys(isolated_config):
    update_config({"scrcpy": {"custom": "x"}})
    full = load_config()
    full["scrcpy"].pop("custom")
    replace_config(full)
    assert "custom" not in load_config()["scrcpy"]


def test_save_config_is_atomic(isolated_config):
    """BUG-11: no truncate-then-write window that can leave a half file."""
    save_config(copy.deepcopy(DEFAULT_CONFIG))
    original = isolated_config.read_text(encoding="utf-8")

    payload = copy.deepcopy(DEFAULT_CONFIG)
    payload["language"] = "ru"
    save_config(payload)

    assert json.loads(isolated_config.read_text(encoding="utf-8"))["language"] == "ru"
    assert json.loads(original)["language"] == "en"
    # No temp files left behind.
    assert list(isolated_config.parent.glob("*.tmp")) == []


def test_invalid_port_falls_back():
    merged = _deep_merge(copy.deepcopy(DEFAULT_CONFIG), {"web": {"port": "banana"}})
    result, _ = _migrate_config(merged)
    assert result["web"]["port"] == 6969
