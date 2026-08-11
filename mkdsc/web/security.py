"""Hardening for the local API.

The backend drives a connected phone: it lists and deletes files, runs adb
shell commands and takes screenshots. Anything that can talk to it owns the
device, so requests pass three independent checks:

1. ``Origin`` allow-list -- stops a page on any other site from calling us
   through the browser (cross-origin ``fetch`` always sends ``Origin``).
2. ``Host`` allow-list -- stops DNS rebinding from mapping an attacker domain
   onto 127.0.0.1.
3. Shared token -- the frontend gets it either injected into the page we serve
   ourselves or, in the desktop build, from the Tauri launcher over an env var.
"""
import ipaddress
import os
import secrets
from urllib.parse import urlsplit

TOKEN_ENV = "MKDSC_API_TOKEN"
TOKEN_HEADER = "x-mkdsc-token"
TOKEN_QUERY = "token"

# Origins the Tauri v2 webview uses depending on platform.
TAURI_ORIGINS = frozenset({
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
})

_LOOPBACK_HOSTNAMES = frozenset({"localhost", "127.0.0.1", "::1"})

# Loopback on any port (vite dev server, the panel itself) plus the Tauri
# webview origins. Everything else is rejected.
CORS_ORIGIN_REGEX = (
    r"^(?:https?://(?:localhost|127\.0\.0\.1|\[::1\])(?::\d+)?"
    r"|tauri://localhost"
    r"|https?://tauri\.localhost)$"
)

_token = None


def get_api_token() -> str:
    """Return the shared token, generating (and exporting) one on first use."""
    global _token
    if _token is None:
        env_token = (os.environ.get(TOKEN_ENV) or "").strip()
        _token = env_token or secrets.token_urlsafe(32)
        # Exported so child processes and the Tauri launcher see the same value.
        os.environ[TOKEN_ENV] = _token
    return _token


def _hostname(value: str) -> str:
    """Strip the port and IPv6 brackets from a ``host[:port]`` string."""
    host = (value or "").strip().lower()
    if host.startswith("["):
        end = host.find("]")
        return host[1:end] if end > 0 else host[1:]
    if host.count(":") == 1:
        host = host.split(":", 1)[0]
    return host


def _is_loopback(host: str) -> bool:
    if not host:
        return False
    if host in _LOOPBACK_HOSTNAMES:
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def _is_ip_literal(host: str) -> bool:
    try:
        ipaddress.ip_address(host)
        return True
    except ValueError:
        return False


def is_allowed_origin(origin: str) -> bool:
    """True for our own frontend origins, false for every other site."""
    origin = (origin or "").strip()
    if not origin:
        # Same-origin navigations and non-browser clients send no Origin.
        return True
    if origin.lower() == "null":
        # Sandboxed iframe or file:// -- never one of ours.
        return False
    if origin.lower() in TAURI_ORIGINS:
        return True
    parts = urlsplit(origin)
    if parts.scheme not in ("http", "https"):
        return False
    return _is_loopback(_hostname(parts.netloc))


def is_allowed_host(host_header: str, bind_host: str = "") -> bool:
    """Reject Host headers that are not loopback / a literal IP we listen on.

    DNS rebinding needs a *name* that resolves to 127.0.0.1, so allowing only
    loopback names and bare IP literals closes it while keeping LAN access
    working for users who deliberately bind to 0.0.0.0.
    """
    host = _hostname(host_header)
    if not host:
        return False
    if _is_loopback(host):
        return True
    if _is_ip_literal(host):
        # Bare IP means the client typed the address; rebinding cannot use it.
        return not _is_loopback(_hostname(bind_host)) if bind_host else True
    return False


def extract_token(headers, query_params) -> str:
    """Token from the header, falling back to ?token= for WS and <img>/<a>."""
    token = headers.get(TOKEN_HEADER) or ""
    if not token:
        token = query_params.get(TOKEN_QUERY) or ""
    return token.strip()


def token_matches(candidate: str) -> bool:
    return secrets.compare_digest(candidate or "", get_api_token())
