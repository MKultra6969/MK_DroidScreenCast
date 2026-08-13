import pytest

from mkdsc.web.file_manager import (
    _is_protected_path,
    _list_target,
    _normalize_path,
    _safe_filename,
    _shell_string,
    parse_ls_output,
)


def test_arguments_with_spaces_are_quoted():
    """BUG-08: adb shell re-splits argv, so 'My Folder' became two paths."""
    assert _shell_string(["rm", "-rf", "/sdcard/My Folder"]) == "rm -rf '/sdcard/My Folder'"


def test_plain_string_command_passes_through():
    assert _shell_string("ls -la /sdcard") == "ls -la /sdcard"


@pytest.mark.parametrize(
    "path",
    ["/", "//sdcard", "/sdcard", "/sdcard/.", "/sdcard/..", "/storage/emulated/0",
     "/system/bin", "/data", "/proc/1", "/dev/block"],
)
def test_protected_paths_cannot_be_bypassed(path):
    """BUG-06: the old check compared raw strings against a small exact set."""
    assert _is_protected_path(path)


@pytest.mark.parametrize("path", ["/sdcard/DCIM", "/sdcard/Download/a.txt", "/data/local/tmp/x"])
def test_normal_paths_are_allowed(path):
    assert not _is_protected_path(path)


def test_normalize_collapses_traversal():
    assert _normalize_path("/sdcard/a/../b") == "/sdcard/b"
    assert _normalize_path("//sdcard//") == "/sdcard"


def test_safe_filename_strips_traversal():
    """BUG-05: multipart filename is fully client-controlled."""
    assert _safe_filename("../../evil.bat") == "evil.bat"
    assert _safe_filename("..\\..\\evil.bat") == "evil.bat"
    with pytest.raises(Exception):
        _safe_filename("..")
    with pytest.raises(Exception):
        _safe_filename("")


def test_symlink_name_excludes_target():
    """BUG-15: /sdcard is a symlink; the target leaked into name and path."""
    line = "lrwxrwxrwx 1 root root 21 2026-01-11 10:00 sdcard -> /storage/self/primary"
    entry = parse_ls_output(line, "/")[0]
    assert entry.name == "sdcard"
    assert entry.path == "/sdcard"
    assert entry.is_link
    assert entry.link_target == "/storage/self/primary"


def test_names_with_spaces_survive_parsing():
    line = "-rw-rw---- 1 root sdcard_rw 12 2026-01-11 10:00 My Documents"
    entry = parse_ls_output(line, "/sdcard")[0]
    assert entry.name == "My Documents"
    assert entry.path == "/sdcard/My Documents"


def test_directories_are_flagged():
    line = "drwxrwx--- 4 root sdcard_rw 3452 2026-01-11 10:00 DCIM"
    entry = parse_ls_output(line, "/sdcard")[0]
    assert entry.is_dir
    assert not entry.is_link


def test_dot_entries_are_skipped():
    output = "\n".join([
        "total 8",
        "drwxrwx--- 4 root sdcard_rw 3452 2026-01-11 10:00 .",
        "drwxrwx--- 4 root sdcard_rw 3452 2026-01-11 10:00 ..",
        "drwxrwx--- 4 root sdcard_rw 3452 2026-01-11 10:00 DCIM",
    ])
    entries = parse_ls_output(output, "/sdcard")
    assert [entry.name for entry in entries] == ["DCIM"]


def test_listing_dereferences_symlinked_directories():
    """`ls -la /sdcard` describes the symlink itself; the trailing slash opens it."""
    assert _list_target("/sdcard") == "/sdcard/"
    assert _list_target("/sdcard/") == "/sdcard/"
    assert _list_target("/sdcard/DCIM") == "/sdcard/DCIM/"
    # The root must not turn into "//".
    assert _list_target("/") == "/"
