import hashlib
import logging
import os
import platform
import re
import shutil
import subprocess
import tarfile
import zipfile
from pathlib import Path

import requests

from .paths import BASE_DIR, DOWNLOADS_DIR

logger = logging.getLogger("mkdsc.tools")

_TOOL_CACHE = {}

# Ни один вызов adb не должен висеть вечно: он блокирует вызывающий поток,
# а в вебе — обработчик запроса.
DEFAULT_CMD_TIMEOUT = 20
TIMEOUT_RETURNCODE = 124

# Каталоги, в которые бессмысленно спускаться в поисках adb/scrcpy.
_IGNORED_SEARCH_DIRS = frozenset({
    "node_modules",
    "target",
    "dist",
    "dist-tauri",
    "site-packages",
    "__pycache__",
    "recordings",
    "screenshots",
    "logs",
})
_MAX_SEARCH_DEPTH = 5

_ANDROID_REPO = "https://dl.google.com/android/repository"

# Качаем конкретную ревизию platform-tools, а не 'platform-tools-latest-*.zip':
# у 'latest' нет опубликованной контрольной суммы и содержимое меняется под
# нами, так что проверить нечего и версия у каждого своя. Версионные архивы на
# dl.google.com лежат бессрочно (r33 доступен до сих пор), поэтому пин безопасен.
#
# Google публикует суммы только в repository2-3.xml и только SHA-1, поэтому
# SHA-256 зафиксированы здесь. Посчитаны с этих самых файлов; их SHA-1 сверены
# с манифестом. Порядок обновления ревизии описан в docs/build.md.
PLATFORM_TOOLS_REVISION = "37.0.1"
PLATFORM_TOOLS_URLS = {
    "windows": f"{_ANDROID_REPO}/platform-tools_r{PLATFORM_TOOLS_REVISION}-win.zip",
    "linux": f"{_ANDROID_REPO}/platform-tools_r{PLATFORM_TOOLS_REVISION}-linux.zip",
    "darwin": f"{_ANDROID_REPO}/platform-tools_r{PLATFORM_TOOLS_REVISION}-darwin.zip",
}
PLATFORM_TOOLS_SHA256 = {
    "windows": "45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99",
    "linux": "d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1",
    "darwin": "ee39ad5967e95c2a07f04dbcbde96b1a0c916ba376096db5d2f498b7727a5d1d",
}

# Раньше здесь был releases/latest, и каждая новая установка получала версию,
# на которой приложение никто не проверял: у разработчика лежал 3.3.4, а
# пользователь получал 4.x. Приложение управляет scrcpy строкой аргументов,
# поэтому переименованный флаг в мажорном релизе ломает продукт у всех новых
# пользователей сразу.
SCRCPY_PINNED_VERSION = "4.1"
SCRCPY_RELEASES_API = "https://api.github.com/repos/Genymobile/scrcpy/releases"
SCRCPY_CHECKSUMS_ASSET = "SHA256SUMS.txt"

# Диапазон, на котором проверен набор флагов из mkdsc/web/server.py
# (launch_scrcpy_api и start_recording) и mkdsc/cli/app.py. Сверено по
# app/src/cli.c тегов v3.3.4 и v4.1: ни один используемый флаг не удалён и не
# переименован, значения --keyboard (uhid/sdk/aoa) и --audio-source
# (output/mic) на месте. В 4.x выпали только давно устаревшие псевдонимы
# (--bit-rate, --display, --no-display, --hid-keyboard), которых здесь нет.
#
# Верхняя граница сравнивается по major.minor: патч-релизы внутри 4.1 набор
# флагов не меняют.
SCRCPY_VERIFIED_MIN = (3, 3, 4)
SCRCPY_VERIFIED_MAX = (4, 1)


def run_cmd(cmd, cwd=None, show_output=False, timeout=DEFAULT_CMD_TIMEOUT):
    if show_output:
        print("> " + " ".join(map(str, cmd)))

    try:
        result = subprocess.run(
            cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired as exc:
        result = subprocess.CompletedProcess(
            args=cmd,
            returncode=TIMEOUT_RETURNCODE,
            stdout=exc.stdout or "",
            stderr=f"Command timed out after {timeout} seconds",
        )

    if show_output:
        if result.stdout:
            print(result.stdout.strip())
        if result.stderr:
            print(result.stderr.strip())

    return result


def _walk_for_file(root: Path, filename: str, max_depth: int = _MAX_SEARCH_DEPTH):
    """Ищет файл вглубь, но не заходит в node_modules/.venv/target и т.п.

    Раньше сюда попадал корень проекта целиком, и один промах кэша стоил
    обхода десятков тысяч файлов.
    """
    stack = [(root, 0)]
    while stack:
        current, depth = stack.pop()
        try:
            entries = list(current.iterdir())
        except OSError:
            continue
        for entry in entries:
            try:
                is_dir = entry.is_dir()
            except OSError:
                continue
            if is_dir:
                if depth >= max_depth:
                    continue
                if entry.name in _IGNORED_SEARCH_DIRS or entry.name.startswith("."):
                    continue
                stack.append((entry, depth + 1))
            elif entry.name == filename:
                return entry
    return None


def _find_exe(filename):
    search_roots = [
        DOWNLOADS_DIR,
        BASE_DIR / "downloads",
        BASE_DIR / "bin",
        BASE_DIR,
    ]
    seen = set()
    for root in search_roots:
        resolved = str(root)
        if resolved in seen or not root.exists():
            continue
        seen.add(resolved)
        local_path = _walk_for_file(root, filename)
        if local_path:
            return local_path
    path = shutil.which(filename)
    if path:
        return Path(path)
    return None


# Прогресс первичной подготовки инструментов: сервер отдаёт его в
# /api/bootstrap/status, чтобы UI показывал загрузку, а не вечную крутилку.
_BOOTSTRAP_STATUS = {
    "stage": "idle",
    "tool": "",
    "downloaded_bytes": 0,
    "total_bytes": 0,
    # Версия найденного scrcpy и предупреждение, если она вне проверенного
    # диапазона. Пустая строка — предупреждать не о чем.
    "scrcpy_version": "",
    "scrcpy_version_warning": "",
}

# Запуск `scrcpy --version` на каждое разрешение пути обходится в лишний
# процесс, а ответ не меняется — держим результат по пути к бинарю.
_SCRCPY_VERSION_CACHE = {}


def get_bootstrap_status():
    return dict(_BOOTSTRAP_STATUS)


def _set_bootstrap_status(**values):
    _BOOTSTRAP_STATUS.update(values)


def _download_file(url, dest, tool_name=""):
    with requests.get(url, stream=True, timeout=30) as response:
        response.raise_for_status()
        try:
            total = int(response.headers.get("content-length") or 0)
        except (TypeError, ValueError):
            total = 0
        _set_bootstrap_status(
            stage="downloading", tool=tool_name, downloaded_bytes=0, total_bytes=total
        )
        received = 0
        with open(dest, "wb") as handle:
            for chunk in response.iter_content(chunk_size=8192):
                if chunk:
                    handle.write(chunk)
                    received += len(chunk)
                    _set_bootstrap_status(downloaded_bytes=received)


def _sha256_file(path) -> str:
    """SHA-256 файла, читая его кусками: архивы весят десятки мегабайт."""
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _verify_sha256(path: Path, expected: str) -> None:
    """Сверяет контрольную сумму архива ДО распаковки.

    Раньше скачанное сразу уходило в распаковщик: подмену архива (MITM,
    компрометация зеркала, кривой редирект) заметить было нечем. Битый файл
    удаляем, чтобы следующий запуск не пытался распаковать его повторно.

    :raises RuntimeError: сумма не совпала.
    """
    actual = _sha256_file(path)
    if actual.lower() != (expected or "").strip().lower():
        path.unlink(missing_ok=True)
        raise RuntimeError(
            f"Checksum mismatch for {path.name}: "
            f"expected {expected}, got {actual}. Download was discarded."
        )


def _parse_sha256sums(text, wanted_name):
    """Достаёт сумму нужного файла из ``SHA256SUMS.txt``.

    Формат sha256sum: ``<hex>  <имя>``; в бинарном режиме имя идёт с ``*``.
    """
    for line in (text or "").splitlines():
        parts = line.split()
        if len(parts) != 2:
            continue
        digest, name = parts
        if name.lstrip("*") != wanted_name:
            continue
        digest = digest.strip().lower()
        if len(digest) == 64 and all(char in "0123456789abcdef" for char in digest):
            return digest
    return None


def _assert_within(dest_dir: Path, member_name: str) -> None:
    """Не даём записи вида '../../Startup/evil.bat' уйти из целевой папки."""
    name = (member_name or "").replace("\\", "/")
    if not name or name.startswith("/") or (len(name) > 1 and name[1] == ":"):
        raise ValueError(f"Unsafe archive entry: {member_name!r}")
    target = (dest_dir / name).resolve()
    if target != dest_dir and not target.is_relative_to(dest_dir):
        raise ValueError(f"Unsafe archive entry: {member_name!r}")


def _restore_zip_modes(zf: zipfile.ZipFile, dest_dir: Path) -> None:
    """Возвращает файлам права Unix, которые теряет ``extractall``.

    ``zipfile`` не применяет режим из архива — всё извлекается по umask
    (обычно 0644). adb приезжает именно zip-архивом, поэтому на Linux
    ``downloads/platform-tools/adb`` оставался без бита исполнения:
    ``_verify_tool`` падал с PermissionError, приложение решало, что adb
    сломан, качало архив заново — и так каждый запуск. На чистой машине без
    системного adb продукт не работал вообще.

    Режим лежит в старших 16 битах ``external_attr`` (то же значение, что
    ``st_mode``).
    """
    for member in zf.infolist():
        if member.is_dir():
            continue
        mode = (member.external_attr >> 16) & 0o777
        if not mode:
            # Архив собран не на Unix — поля с правами нет, оставляем umask.
            continue
        target = dest_dir / member.filename
        try:
            target.chmod(mode)
        except OSError:
            # Не повод валить установку: ниже есть страховка _ensure_executable.
            continue


def safe_extract_zip(archive_path: Path, dest_dir: Path) -> None:
    dest_dir = Path(dest_dir).resolve()
    dest_dir.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive_path, "r") as zf:
        for member in zf.infolist():
            _assert_within(dest_dir, member.filename)
        zf.extractall(dest_dir)
        if os.name != "nt":
            # На Windows режимы файлов семантики не имеют.
            _restore_zip_modes(zf, dest_dir)


def safe_extract_tar(archive_path: Path, dest_dir: Path, mode: str = "r:gz") -> None:
    dest_dir = Path(dest_dir).resolve()
    dest_dir.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive_path, mode) as tf:
        if hasattr(tarfile, "data_filter"):
            # 3.12+: отбрасывает абсолютные пути, '..', устройства и симлинки наружу.
            tf.extractall(dest_dir, filter="data")
            return
        for member in tf.getmembers():
            if member.issym() or member.islnk():
                _assert_within(dest_dir, member.linkname)
            _assert_within(dest_dir, member.name)
        tf.extractall(dest_dir)


def _extract_archive(archive_path):
    name = archive_path.name.lower()
    if name.endswith(".zip"):
        safe_extract_zip(archive_path, DOWNLOADS_DIR)
        return
    if name.endswith(".tar.gz") or name.endswith(".tgz"):
        safe_extract_tar(archive_path, DOWNLOADS_DIR)
        return
    raise ValueError(f"Unsupported archive format: {archive_path.name}")


def _download_and_extract(name, url, archive_name=None, expected_sha256=None):
    archive_name = archive_name or f"{name}.zip"
    archive_path = DOWNLOADS_DIR / archive_name
    _download_file(url, archive_path, tool_name=name)
    if expected_sha256:
        _set_bootstrap_status(stage="verifying", tool=name)
        _verify_sha256(archive_path, expected_sha256)
    _set_bootstrap_status(stage="extracting", tool=name)
    _extract_archive(archive_path)
    archive_path.unlink(missing_ok=True)


def _platform_key():
    system = platform.system().lower()
    if system.startswith("win"):
        return "windows"
    if system.startswith("darwin"):
        return "darwin"
    if system.startswith("linux"):
        return "linux"
    return system


def _is_64bit():
    return platform.architecture()[0] == "64bit"


def _arch_tokens():
    machine = platform.machine().lower()
    if machine in {"amd64", "x86_64", "x64"}:
        return ["x86_64", "amd64"]
    if machine in {"arm64", "aarch64"}:
        return ["arm64", "aarch64"]
    if machine in {"armv7l", "armv7"}:
        return ["armv7", "armv7l"]
    if machine in {"i386", "i686", "x86"}:
        return ["x86", "i386", "i686"]
    return [machine]


def _select_scrcpy_asset(assets):
    system = _platform_key()
    candidates = []

    for asset in assets:
        name = (asset.get("name") or "").lower()
        if not name or "server" in name:
            continue
        if system == "windows" and "scrcpy-win" in name:
            if name.endswith(".zip"):
                candidates.append(asset)
        elif system == "linux" and "scrcpy-linux" in name:
            if name.endswith(".tar.gz") or name.endswith(".tgz") or name.endswith(".zip"):
                candidates.append(asset)
        elif system == "darwin" and ("scrcpy-macos" in name or "scrcpy-mac" in name):
            candidates.append(asset)

    if not candidates:
        return None

    if system == "windows":
        target = "win64" if _is_64bit() else "win32"
        for asset in candidates:
            if target in (asset.get("name") or "").lower():
                return asset
        return candidates[0]

    arch_tokens = _arch_tokens()
    for token in arch_tokens:
        for asset in candidates:
            if token in (asset.get("name") or "").lower():
                return asset

    return candidates[0]


def scrcpy_target_version():
    """Версия scrcpy, которую надо поставить.

    ``MKDSC_SCRCPY_VERSION`` позволяет взять свою версию, не пересобирая
    приложение (например, откатиться, если в закреплённой нашёлся баг).
    Ведущая ``v`` необязательна.
    """
    value = (os.environ.get("MKDSC_SCRCPY_VERSION") or "").strip().lstrip("vV")
    return value or SCRCPY_PINNED_VERSION


def _scrcpy_release_url():
    return f"{SCRCPY_RELEASES_API}/tags/v{scrcpy_target_version()}"


def _fetch_scrcpy_release():
    """Ассеты закреплённого релиза scrcpy.

    :returns: ``(assets, checksums_url)``, где ``assets`` — список
        ``{"name", "url"}`` без файла сумм, а ``checksums_url`` — ссылка на
        ``SHA256SUMS.txt`` или ``None``, если релиз его не публикует.
    """
    response = requests.get(_scrcpy_release_url(), timeout=10)
    response.raise_for_status()
    payload = response.json()
    assets = []
    checksums_url = None
    for asset in payload.get("assets", []) or []:
        name = asset.get("name")
        url = asset.get("browser_download_url")
        if name == SCRCPY_CHECKSUMS_ASSET:
            checksums_url = url
            continue
        assets.append({"name": name, "url": url})
    return assets, checksums_url


def _fetch_scrcpy_checksum(checksums_url, archive_name):
    """SHA-256 конкретного ассета из ``SHA256SUMS.txt`` релиза.

    Подпись ``SHA256SUMS.txt.asc`` намеренно не проверяется: для этого нужен
    вшитый публичный ключ и работа с ключевым хранилищем — отдельная задача.
    """
    if not checksums_url:
        return None
    response = requests.get(checksums_url, timeout=10)
    response.raise_for_status()
    return _parse_sha256sums(response.text, archive_name)


_VERSION_RE = re.compile(r"(\d+)\.(\d+)(?:\.(\d+))?")


def parse_scrcpy_version(output):
    """Версия из вывода ``scrcpy --version`` как кортеж ``(major, minor, micro)``.

    Первая строка вывода выглядит так::

        scrcpy 4.1 <https://github.com/Genymobile/scrcpy>

    :returns: кортеж или ``None``, если версию разобрать не удалось.
    """
    for line in (output or "").splitlines():
        line = line.strip()
        if not line.lower().startswith("scrcpy"):
            continue
        match = _VERSION_RE.search(line)
        if match:
            major, minor, micro = match.groups()
            return (int(major), int(minor), int(micro or 0))
    return None


def scrcpy_version_warning(version):
    """Текст предупреждения, если версия вне проверенного диапазона.

    Пустая строка означает «всё в порядке». Предупреждение намеренно не
    блокирует запуск: пользователь мог поставить свою версию сознательно.
    """
    if version is None:
        return ""
    if version >= SCRCPY_VERIFIED_MIN and version[:2] <= SCRCPY_VERIFIED_MAX:
        return ""
    found = ".".join(str(part) for part in version)
    tested_min = ".".join(str(part) for part in SCRCPY_VERIFIED_MIN)
    return (
        f"scrcpy {found} is outside the tested range "
        f"{tested_min}-{SCRCPY_PINNED_VERSION}; some options may not work"
    )


def _record_scrcpy_version(path):
    """Разбирает ``scrcpy --version`` и кладёт результат в статус загрузки.

    Вызывается на каждом разрешении пути к scrcpy, поэтому результат
    запоминается: лишний запуск процесса на каждый запрос ни к чему.
    """
    key = str(path)
    if key in _SCRCPY_VERSION_CACHE:
        return _SCRCPY_VERSION_CACHE[key]

    result = run_cmd([str(path), "--version"], show_output=False)
    version = parse_scrcpy_version(f"{result.stdout}\n{result.stderr}")
    text = ".".join(str(part) for part in version) if version else ""
    warning = scrcpy_version_warning(version)
    if warning:
        logger.warning(warning)

    _SCRCPY_VERSION_CACHE[key] = (text, warning)
    _set_bootstrap_status(scrcpy_version=text, scrcpy_version_warning=warning)
    return text, warning


def _ensure_executable(path):
    """Доставляет бит исполнения на POSIX, если его нет.

    Страховка к :func:`_restore_zip_modes`: тот полагается на ``external_attr``,
    а его в архиве может не оказаться (архив собран на Windows, пересобран
    зеркалом и т.п.). Тогда бинарь окажется 0644 и просто не запустится.
    Дешевле поправить режим здесь, чем ловить PermissionError из subprocess.

    На Windows режимы файлов семантики не имеют — выходим сразу.
    """
    if os.name == "nt":
        return
    try:
        mode = Path(path).stat().st_mode
    except OSError:
        return
    if mode & 0o111:
        return
    try:
        Path(path).chmod(mode | 0o111)
    except OSError:
        pass


def _verify_tool(path, args):
    try:
        result = run_cmd([str(path)] + args, show_output=False)
        return result.returncode == 0
    except Exception:
        return False


def _cache_tool(name, path):
    if path:
        _TOOL_CACHE[name] = Path(path)


def _cached_tool(name):
    path = _TOOL_CACHE.get(name)
    if path and Path(path).exists():
        return Path(path)
    return None


def _resolve_env_tool(env_key):
    value = os.environ.get(env_key)
    if not value:
        return None
    path = Path(value)
    return path if path.exists() else None


def _ensure_adb():
    DOWNLOADS_DIR.mkdir(parents=True, exist_ok=True)

    override = _resolve_env_tool("MKDSC_ADB_PATH")
    if override and _verify_tool(override, ["version"]):
        _cache_tool("adb", override)
        return override

    cached = _cached_tool("adb")
    if cached and _verify_tool(cached, ["version"]):
        return cached

    adb_name = "adb.exe" if os.name == "nt" else "adb"
    adb_path = _find_exe(adb_name)
    if adb_path:
        # Права чиним ДО проверки: иначе распакованный ранее 0644-adb не
        # запустится, и мы уйдём качать архив по кругу.
        _ensure_executable(adb_path)
    if not adb_path or not _verify_tool(adb_path, ["version"]):
        platform_key = _platform_key()
        platform_url = PLATFORM_TOOLS_URLS.get(platform_key)
        if not platform_url:
            raise RuntimeError(f"Unsupported platform for adb: {platform_key}")
        _download_and_extract(
            "platform-tools",
            platform_url,
            archive_name=f"platform-tools-{PLATFORM_TOOLS_REVISION}-{platform_key}.zip",
            expected_sha256=PLATFORM_TOOLS_SHA256.get(platform_key),
        )
        adb_path = _find_exe(adb_name)
        if not adb_path:
            raise FileNotFoundError(f"{adb_name} not found after download")
        _ensure_executable(adb_path)

    _cache_tool("adb", adb_path)
    return adb_path


def _ensure_scrcpy():
    DOWNLOADS_DIR.mkdir(parents=True, exist_ok=True)

    override = _resolve_env_tool("MKDSC_SCRCPY_PATH")
    if override and _verify_tool(override, ["--version"]):
        _cache_tool("scrcpy", override)
        # Свой scrcpy — как раз тот случай, ради которого версия и проверяется.
        _record_scrcpy_version(override)
        return override

    cached = _cached_tool("scrcpy")
    if cached and _verify_tool(cached, ["--version"]):
        _record_scrcpy_version(cached)
        return cached

    scrcpy_name = "scrcpy.exe" if os.name == "nt" else "scrcpy"
    scrcpy_path = _find_exe(scrcpy_name)
    if scrcpy_path:
        # См. комментарий в _ensure_adb: режим чиним до проверки запуска.
        _ensure_executable(scrcpy_path)
    if not scrcpy_path or not _verify_tool(scrcpy_path, ["--version"]):
        assets, checksums_url = _fetch_scrcpy_release()
        asset = _select_scrcpy_asset(assets)
        if not asset or not asset.get("url"):
            raise FileNotFoundError("scrcpy archive not found for this platform")
        archive_name = asset.get("name") or "scrcpy.zip"
        lowered = archive_name.lower()
        if not (lowered.endswith(".zip") or lowered.endswith(".tar.gz") or lowered.endswith(".tgz")):
            raise RuntimeError(f"Unsupported scrcpy archive format: {archive_name}")
        expected_sha256 = _fetch_scrcpy_checksum(checksums_url, archive_name)
        if not expected_sha256:
            # Падаем закрыто: без опубликованной суммы проверить нечего, а
            # молча ставить непроверенный бинарь — ровно то, что чинится.
            raise RuntimeError(
                f"No SHA-256 checksum published for {archive_name}. "
                f"Set MKDSC_SCRCPY_VERSION to a release that ships "
                f"{SCRCPY_CHECKSUMS_ASSET}, or point MKDSC_SCRCPY_PATH at your "
                f"own scrcpy."
            )
        _download_and_extract(
            "scrcpy",
            asset["url"],
            archive_name=archive_name,
            expected_sha256=expected_sha256,
        )
        scrcpy_path = _find_exe(scrcpy_name)
        if not scrcpy_path:
            raise FileNotFoundError(f"{scrcpy_name} not found after download")
        _ensure_executable(scrcpy_path)

    _cache_tool("scrcpy", scrcpy_path)
    _record_scrcpy_version(scrcpy_path)
    return scrcpy_path


def ensure_tools():
    _set_bootstrap_status(stage="checking", tool="adb")
    adb_path = _ensure_adb()
    _set_bootstrap_status(stage="checking", tool="scrcpy")
    scrcpy_path = _ensure_scrcpy()
    _set_bootstrap_status(stage="ready", tool="", downloaded_bytes=0, total_bytes=0)
    return adb_path, scrcpy_path


def get_tool_path(name):
    tool = (name or "").strip().lower()
    if tool == "adb":
        return _ensure_adb()
    if tool == "scrcpy":
        return _ensure_scrcpy()
    raise ValueError(f"Unknown tool requested: {name}")


def start_adb_server(adb_path):
    """start-server идемпотентен.

    Раньше здесь был kill-server, который на каждом запуске отваливал
    Android Studio / Flutter / любой другой инструмент пользователя.
    """
    run_cmd([str(adb_path), "start-server"], show_output=False, timeout=30)


def restart_adb_server(adb_path):
    """Явный перезапуск — только по кнопке в диагностике."""
    run_cmd([str(adb_path), "kill-server"], show_output=False, timeout=30)
    run_cmd([str(adb_path), "start-server"], show_output=False, timeout=30)


def stop_adb_server(adb_path):
    """Оставляем общий adb-сервер жить: его могут использовать другие программы."""
    return


def get_connected_devices(adb_path):
    result = run_cmd([str(adb_path), "devices"], show_output=False)
    lines = result.stdout.strip().split("\n")[1:]

    devices = []
    for line in lines:
        if "\t" in line:
            serial, status = line.split("\t")
            devices.append({"serial": serial, "status": status})

    return devices


def _adb_cmd(adb_path, serial, *args):
    cmd = [str(adb_path)]
    if serial:
        cmd.extend(["-s", str(serial)])
    cmd.extend(args)
    return cmd


def adb_shell_get(adb_path, prop, serial=None):
    result = run_cmd(_adb_cmd(adb_path, serial, "shell", "getprop", prop), show_output=False)
    if result.returncode != 0:
        return None
    value = result.stdout.strip()
    return value or None


def get_device_info(adb_path, serial=None):
    return {
        "model": adb_shell_get(adb_path, "ro.product.model", serial),
        "android_version": adb_shell_get(adb_path, "ro.build.version.release", serial),
    }


def get_device_wifi_ip(adb_path, serial=None):
    cmd = _adb_cmd(adb_path, serial, "shell", "ip", "addr", "show", "wlan0")
    result = run_cmd(cmd, show_output=False)
    if result.returncode != 0:
        return None

    for line in result.stdout.split("\n"):
        if "inet " in line and "inet6" not in line:
            parts = line.strip().split()
            if len(parts) >= 2:
                return parts[1].split("/")[0]
    return None


def get_setting(adb_path, namespace, key, serial=None):
    result = run_cmd(
        _adb_cmd(adb_path, serial, "shell", "settings", "get", namespace, key),
        show_output=False,
    )
    if result.returncode != 0:
        return None
    value = result.stdout.strip()
    return value if value != "null" else None


def put_setting(adb_path, namespace, key, value, serial=None):
    """Возвращает True, если настройка применилась.

    Без -s adb с двумя устройствами отвечает 'more than one device', и
    «не гасить экран» молча не работало.
    """
    result = run_cmd(
        _adb_cmd(adb_path, serial, "shell", "settings", "put", namespace, key, str(value)),
        show_output=False,
    )
    return result.returncode == 0


def delete_setting(adb_path, namespace, key, serial=None):
    result = run_cmd(
        _adb_cmd(adb_path, serial, "shell", "settings", "delete", namespace, key),
        show_output=False,
    )
    return result.returncode == 0
