import re
from datetime import datetime

import requests

from .constants import API_LATEST_RELEASE


def _parse_version(version):
    if not version:
        return ()
    cleaned = version.strip().lower().lstrip("v")
    parts = re.split(r"[^0-9]+", cleaned)
    return tuple(int(part) for part in parts if part.isdigit())


def is_newer(current_version, latest_version):
    return _parse_version(latest_version) > _parse_version(current_version)


def fetch_latest_release(api_url=API_LATEST_RELEASE):
    response = requests.get(api_url, timeout=10)
    response.raise_for_status()
    payload = response.json()
    return {
        "tag": payload.get("tag_name"),
        "name": payload.get("name"),
        "body": payload.get("body") or "",
        "published_at": payload.get("published_at"),
        "zipball_url": payload.get("zipball_url"),
        "html_url": payload.get("html_url"),
        "assets": [
            {
                "name": asset.get("name"),
                "url": asset.get("browser_download_url"),
                "size": asset.get("size"),
            }
            for asset in payload.get("assets", []) or []
        ],
        "fetched_at": datetime.utcnow().isoformat(),
    }
