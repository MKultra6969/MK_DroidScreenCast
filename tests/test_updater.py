import mkdsc.updater as updater
from mkdsc.constants import RELEASES_URL


def test_self_updating_code_is_gone():
    """BUG-32: the source-zipball-over-BASE_DIR updater must not come back.

    Desktop updates go through tauri-plugin-updater; everything else points at
    the release page.
    """
    for name in ("apply_update", "_copy_tree", "_extract_archive", "_download_file"):
        assert not hasattr(updater, name), f"{name} should no longer exist"


def test_check_exposes_release_url(monkeypatch):
    monkeypatch.setattr(
        updater,
        "fetch_latest_release",
        lambda _url: {"tag": "v9.9.9", "html_url": "https://example.test/release"},
    )
    info = updater.check_for_updates()
    assert info["update_available"]
    assert info["release_url"] == "https://example.test/release"


def test_check_reports_up_to_date(monkeypatch):
    monkeypatch.setattr(
        updater,
        "fetch_latest_release",
        lambda _url: {"tag": "v0.0.1", "html_url": ""},
    )
    info = updater.check_for_updates()
    assert not info["update_available"]
    assert info["release_url"] == RELEASES_URL


def test_open_release_page_uses_release_url(monkeypatch):
    opened = []
    monkeypatch.setattr(updater.webbrowser, "open", lambda url: opened.append(url) or True)

    result = updater.open_release_page({"html_url": "https://example.test/r/1"})
    assert result["success"]
    assert opened == ["https://example.test/r/1"]


def test_open_release_page_falls_back_to_releases(monkeypatch):
    monkeypatch.setattr(updater.webbrowser, "open", lambda _url: True)
    assert updater.open_release_page(None)["release_url"] == RELEASES_URL


def test_open_release_page_survives_browser_failure(monkeypatch):
    def boom(_url):
        raise RuntimeError("no browser")

    monkeypatch.setattr(updater.webbrowser, "open", boom)
    result = updater.open_release_page(None)
    assert not result["success"]
    assert result["release_url"] == RELEASES_URL
