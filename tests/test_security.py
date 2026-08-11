import pytest
from fastapi import HTTPException

from mkdsc.web.security import (
    get_api_token,
    is_allowed_host,
    is_allowed_origin,
    token_matches,
)
from mkdsc.web.service_commands import validate_custom_command


def blocked(command, confirm=False):
    try:
        validate_custom_command(command, confirm)
        return False
    except HTTPException:
        return True


# --- BUG-01: origin / host -------------------------------------------------

@pytest.mark.parametrize("origin", ["https://evil.com", "http://attacker.local:8080", "null"])
def test_foreign_origins_rejected(origin):
    assert not is_allowed_origin(origin)


@pytest.mark.parametrize(
    "origin",
    ["", "http://localhost:5173", "http://127.0.0.1:6969", "tauri://localhost", "http://tauri.localhost"],
)
def test_own_origins_allowed(origin):
    assert is_allowed_origin(origin)


def test_dns_rebinding_host_rejected():
    assert not is_allowed_host("rebind.evil.com", "127.0.0.1")
    assert not is_allowed_host("rebind.evil.com:6969", "0.0.0.0")


def test_loopback_host_allowed():
    assert is_allowed_host("127.0.0.1:6969", "127.0.0.1")
    assert is_allowed_host("localhost:6969", "127.0.0.1")


def test_lan_ip_only_when_bound_wide():
    assert is_allowed_host("192.168.1.5:6969", "0.0.0.0")
    assert not is_allowed_host("192.168.1.5:6969", "127.0.0.1")


def test_token_comparison():
    assert token_matches(get_api_token())
    assert not token_matches("wrong")
    assert not token_matches("")


# --- BUG-03: command validation -------------------------------------------

@pytest.mark.parametrize(
    "command",
    ["dumpsys battery", "getprop", "cat /proc/meminfo", "dumpsys | grep format", "df -h"],
)
def test_diagnostics_run_without_confirmation(command):
    """The old blocklist rejected 'dumpsys | grep format' as 'dangerous'."""
    assert not blocked(command)


@pytest.mark.parametrize(
    "command",
    ["rm  -rf /", "rm -rf /", "rm -r /sdcard", "rmdir /storage/emulated/0"],
)
def test_protected_deletions_always_refused(command):
    assert blocked(command)
    assert blocked(command, confirm=True)


@pytest.mark.parametrize("command", ["reboot bootloader", "reboot recovery", "fastboot devices"])
def test_bricking_commands_always_refused(command):
    assert blocked(command, confirm=True)


@pytest.mark.parametrize(
    "command",
    ["dd if=/dev/zero of=/sdcard/x", "pm uninstall com.android.chrome", "cd /sdcard && rm -r tmp"],
)
def test_state_changing_commands_need_confirmation(command):
    assert blocked(command)
    assert not blocked(command, confirm=True)


def test_empty_command_rejected():
    assert blocked("")
    assert blocked("   ")
