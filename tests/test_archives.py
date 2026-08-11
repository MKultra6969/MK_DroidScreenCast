import tarfile
import zipfile

import pytest

from mkdsc.tools import safe_extract_tar, safe_extract_zip


def test_zip_slip_is_rejected(tmp_path):
    """BUG-04: '../../Startup/evil.bat' used to be written outside the target."""
    archive = tmp_path / "evil.zip"
    with zipfile.ZipFile(archive, "w") as zf:
        zf.writestr("../../evil.bat", "pwned")

    with pytest.raises(ValueError):
        safe_extract_zip(archive, tmp_path / "out")

    assert not (tmp_path.parent / "evil.bat").exists()


def test_absolute_zip_entry_is_rejected(tmp_path):
    archive = tmp_path / "abs.zip"
    with zipfile.ZipFile(archive, "w") as zf:
        zf.writestr("/etc/passwd", "pwned")

    with pytest.raises(ValueError):
        safe_extract_zip(archive, tmp_path / "out")


def test_valid_zip_still_extracts(tmp_path):
    archive = tmp_path / "ok.zip"
    with zipfile.ZipFile(archive, "w") as zf:
        zf.writestr("platform-tools/adb.exe", "binary")

    dest = tmp_path / "out"
    safe_extract_zip(archive, dest)
    assert (dest / "platform-tools" / "adb.exe").read_text() == "binary"


def test_tar_slip_is_rejected(tmp_path):
    payload = tmp_path / "payload.txt"
    payload.write_text("pwned", encoding="utf-8")

    archive = tmp_path / "evil.tar.gz"
    with tarfile.open(archive, "w:gz") as tf:
        tf.add(payload, arcname="../../evil.txt")

    with pytest.raises(Exception):
        safe_extract_tar(archive, tmp_path / "out")

    assert not (tmp_path.parent / "evil.txt").exists()


def test_valid_tar_still_extracts(tmp_path):
    payload = tmp_path / "scrcpy"
    payload.write_text("binary", encoding="utf-8")

    archive = tmp_path / "ok.tar.gz"
    with tarfile.open(archive, "w:gz") as tf:
        tf.add(payload, arcname="scrcpy-linux/scrcpy")

    dest = tmp_path / "out"
    safe_extract_tar(archive, dest)
    assert (dest / "scrcpy-linux" / "scrcpy").read_text() == "binary"
