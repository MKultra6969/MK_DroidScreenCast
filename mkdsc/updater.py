"""Update checking.

Installing updates is *not* done here any more. The desktop build uses
``tauri-plugin-updater``: signed artifacts, an atomic installer swap and a
proper relaunch. The previous implementation downloaded the GitHub source
zipball and copied it over ``BASE_DIR``, which:

- needed admin rights in an installed app (resource dir under Program Files);
- shipped raw .py files while the app actually runs bin/mkdsc-backend.exe;
- never deleted files removed in the new version;
- could not restart anything.

Everything else (web panel from a clone, CLI) now points the user at the
release page instead of pretending to self-update.
"""
import webbrowser

from .constants import API_LATEST_RELEASE, RELEASES_URL, VERSION
from .versioning import fetch_latest_release, is_newer


def check_for_updates():
    release = fetch_latest_release(API_LATEST_RELEASE)
    latest = release.get("tag")
    update_available = bool(latest and is_newer(VERSION, latest))
    return {
        "current": VERSION,
        "latest": latest,
        "update_available": update_available,
        "release": release,
        "release_url": release.get("html_url") or RELEASES_URL,
    }


def get_release_url(release=None):
    if isinstance(release, dict) and release.get("html_url"):
        return release["html_url"]
    return RELEASES_URL


def open_release_page(release=None):
    """Open the release page in the user's browser.

    Used by the CLI and the from-source menu; the desktop app never gets here
    because the Tauri updater installs in place.
    """
    url = get_release_url(release)
    opened = False
    try:
        opened = bool(webbrowser.open(url))
    except Exception:
        opened = False
    return {
        "success": opened,
        "opened": opened,
        "release_url": url,
    }
