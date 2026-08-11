import copy
import json

import pytest

from mkdsc import config as config_module
from mkdsc.config import (
    DEFAULT_CONFIG,
    ConfigReadOnlyError,
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
    # Переменная могла остаться в окружении разработчика от запуска десктопа —
    # тогда каждый тест на запись падал бы без видимой причины.
    monkeypatch.delenv("MKDSC_CONFIG_READONLY", raising=False)
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


@pytest.fixture
def readonly_mode(monkeypatch):
    """Режим бэкенда внутри десктопного приложения: конфигом владеет Rust."""
    monkeypatch.setenv("MKDSC_CONFIG_READONLY", "1")


def test_save_config_refuses_in_readonly_mode(isolated_config, readonly_mode):
    with pytest.raises(ConfigReadOnlyError):
        save_config(copy.deepcopy(DEFAULT_CONFIG))
    assert not isolated_config.exists()


def test_update_and_replace_refuse_in_readonly_mode(isolated_config, monkeypatch):
    save_config(copy.deepcopy(DEFAULT_CONFIG))
    monkeypatch.setenv("MKDSC_CONFIG_READONLY", "1")

    with pytest.raises(ConfigReadOnlyError):
        update_config({"language": "ru"})
    with pytest.raises(ConfigReadOnlyError):
        replace_config(copy.deepcopy(DEFAULT_CONFIG))

    assert json.loads(isolated_config.read_text(encoding="utf-8"))["language"] == "en"


def test_load_config_skips_write_back_in_readonly_mode(isolated_config, monkeypatch):
    """Обратная запись срабатывает на обычном чтении — её легко не заметить."""
    isolated_config.write_text(json.dumps({"language": "ru"}), encoding="utf-8")
    before = isolated_config.read_text(encoding="utf-8")
    monkeypatch.setenv("MKDSC_CONFIG_READONLY", "1")

    loaded = load_config()

    # Дефолты и миграция доезжают в память...
    assert loaded["language"] == "ru"
    assert loaded["web"]["port"] == 6969
    assert loaded["scrcpy"]["presets"]
    # ...но не в файл.
    assert isolated_config.read_text(encoding="utf-8") == before


def test_load_config_without_file_does_not_create_it_in_readonly_mode(
    isolated_config, readonly_mode
):
    """Первый запуск десктопа: конфига ещё нет, создаст его Rust.

    Раньше эта ветка безусловно сохраняла файл, то есть бэкенд не поднялся бы.
    """
    loaded = load_config()

    assert loaded["web"]["port"] == 6969
    assert not isolated_config.exists()
