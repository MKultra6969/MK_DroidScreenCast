import os
import stat
import zipfile

import pytest

from mkdsc import tools
from mkdsc.tools import (
    _ensure_executable,
    _parse_sha256sums,
    _sha256_file,
    _verify_sha256,
    parse_scrcpy_version,
    safe_extract_zip,
    scrcpy_target_version,
    scrcpy_version_warning,
)

posix_only = pytest.mark.skipif(
    os.name == "nt", reason="file modes have no meaning on Windows"
)


def _write_zip(path, name, payload, external_attr=None):
    with zipfile.ZipFile(path, "w") as zf:
        info = zipfile.ZipInfo(name)
        if external_attr is not None:
            info.external_attr = external_attr
        zf.writestr(info, payload)


# --- exec bit after unzip ---------------------------------------------------


@posix_only
def test_zip_extract_restores_exec_bit(tmp_path):
    """adb приезжает zip-архивом, а zipfile теряет права Unix.

    Без восстановления режима downloads/platform-tools/adb оказывался 0644,
    и на Linux приложение не могло его запустить вообще.
    """
    archive = tmp_path / "platform-tools.zip"
    _write_zip(archive, "platform-tools/adb", "binary", external_attr=0o755 << 16)

    dest = tmp_path / "out"
    safe_extract_zip(archive, dest)

    extracted = dest / "platform-tools" / "adb"
    assert extracted.read_text() == "binary"
    assert extracted.stat().st_mode & stat.S_IXUSR


@posix_only
def test_zip_extract_keeps_non_executable_entries_non_executable(tmp_path):
    """Восстанавливаем именно записанный режим, а не «всем +x»."""
    archive = tmp_path / "platform-tools.zip"
    _write_zip(archive, "platform-tools/NOTICE.txt", "text", external_attr=0o644 << 16)

    dest = tmp_path / "out"
    safe_extract_zip(archive, dest)

    extracted = dest / "platform-tools" / "NOTICE.txt"
    assert not extracted.stat().st_mode & stat.S_IXUSR


def test_zip_without_unix_attrs_still_extracts(tmp_path):
    """Архив, собранный не на Unix: поле прав пустое — это не ошибка."""
    archive = tmp_path / "windows-made.zip"
    _write_zip(archive, "platform-tools/adb.exe", "binary", external_attr=0)

    dest = tmp_path / "out"
    safe_extract_zip(archive, dest)

    assert (dest / "platform-tools" / "adb.exe").read_text() == "binary"


@posix_only
def test_ensure_executable_adds_exec_bit(tmp_path):
    binary = tmp_path / "adb"
    binary.write_text("binary", encoding="utf-8")
    binary.chmod(0o644)

    _ensure_executable(binary)

    assert binary.stat().st_mode & stat.S_IXUSR


@posix_only
def test_ensure_executable_keeps_existing_mode(tmp_path):
    binary = tmp_path / "adb"
    binary.write_text("binary", encoding="utf-8")
    binary.chmod(0o755)

    _ensure_executable(binary)

    assert stat.S_IMODE(binary.stat().st_mode) == 0o755


def test_ensure_executable_survives_missing_file(tmp_path):
    """Отсутствующий путь не должен ронять подготовку инструментов."""
    _ensure_executable(tmp_path / "nope")


# --- checksums --------------------------------------------------------------


def test_verify_sha256_accepts_matching_digest(tmp_path):
    archive = tmp_path / "tool.zip"
    archive.write_bytes(b"payload")

    _verify_sha256(archive, _sha256_file(archive))

    assert archive.exists()


def test_verify_sha256_rejects_and_removes_tampered_archive(tmp_path):
    """Подменённый архив не должен дойти до распаковщика."""
    archive = tmp_path / "tool.zip"
    archive.write_bytes(b"tampered")

    with pytest.raises(RuntimeError):
        _verify_sha256(archive, "0" * 64)

    assert not archive.exists()


def test_parse_sha256sums_finds_requested_asset():
    text = (
        "deacb991ed2509715160ffdc7907e47b4160eb30d1566217e9047fd5b8850cae  "
        "scrcpy-server-v4.1\n"
        "5b12172b3264b2889f4583ee64752ce832e29bc8b1089dca81093459697165db  "
        "scrcpy-win64-v4.1.zip\n"
    )

    assert _parse_sha256sums(text, "scrcpy-win64-v4.1.zip") == (
        "5b12172b3264b2889f4583ee64752ce832e29bc8b1089dca81093459697165db"
    )
    assert _parse_sha256sums(text, "scrcpy-linux-x86_64-v4.1.tar.gz") is None


def test_parse_sha256sums_accepts_binary_mode_marker():
    """sha256sum в бинарном режиме печатает имя с префиксом '*'."""
    digest = "a" * 64
    assert _parse_sha256sums(f"{digest} *scrcpy-win64-v4.1.zip", "scrcpy-win64-v4.1.zip") == digest


def test_parse_sha256sums_ignores_malformed_lines():
    assert _parse_sha256sums("not-a-hash  tool.zip", "tool.zip") is None
    assert _parse_sha256sums("", "tool.zip") is None


def test_platform_tools_are_pinned_with_checksums():
    """Регрессия: URL 'latest' проверить нечем и версия у каждого своя."""
    for key, url in tools.PLATFORM_TOOLS_URLS.items():
        assert "latest" not in url, f"{key} is not pinned to a revision"
        assert tools.PLATFORM_TOOLS_REVISION in url
        digest = tools.PLATFORM_TOOLS_SHA256.get(key)
        assert digest and len(digest) == 64, f"no SHA-256 pinned for {key}"


# --- scrcpy version ---------------------------------------------------------


def test_parse_scrcpy_version_reads_real_output():
    output = "scrcpy 4.1 <https://github.com/Genymobile/scrcpy>\n\nDependencies:\n - SDL: 3.4.12"
    assert parse_scrcpy_version(output) == (4, 1, 0)


def test_parse_scrcpy_version_reads_patch_release():
    assert parse_scrcpy_version("scrcpy 3.3.4 <https://github.com/Genymobile/scrcpy>") == (3, 3, 4)


def test_parse_scrcpy_version_returns_none_on_garbage():
    assert parse_scrcpy_version("command not found") is None
    assert parse_scrcpy_version("") is None


@pytest.mark.parametrize("version", [(3, 3, 4), (4, 0, 0), (4, 1, 0), (4, 1, 2)])
def test_no_warning_inside_tested_range(version):
    assert scrcpy_version_warning(version) == ""


@pytest.mark.parametrize("version", [(2, 7, 0), (3, 3, 3), (5, 0, 0)])
def test_warns_outside_tested_range(version):
    assert "outside the tested range" in scrcpy_version_warning(version)


def test_no_warning_when_version_is_unknown():
    """Не смогли разобрать версию — молчим, а не пугаем пользователя."""
    assert scrcpy_version_warning(None) == ""


def test_scrcpy_version_defaults_to_pinned(monkeypatch):
    monkeypatch.delenv("MKDSC_SCRCPY_VERSION", raising=False)
    assert scrcpy_target_version() == tools.SCRCPY_PINNED_VERSION


def test_scrcpy_release_url_is_pinned_not_latest(monkeypatch):
    """Регрессия: releases/latest отдавал непроверенную версию."""
    monkeypatch.delenv("MKDSC_SCRCPY_VERSION", raising=False)
    url = tools._scrcpy_release_url()
    assert url.endswith(f"/tags/v{tools.SCRCPY_PINNED_VERSION}")
    assert "latest" not in url


def test_scrcpy_version_can_be_overridden(monkeypatch):
    """Пользователь может поставить свою версию, не пересобирая приложение."""
    monkeypatch.setenv("MKDSC_SCRCPY_VERSION", "v3.3.4")
    assert scrcpy_target_version() == "3.3.4"
    assert tools._scrcpy_release_url().endswith("/tags/v3.3.4")
