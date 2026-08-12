//! Поиск внешних инструментов на диске.
//!
//! Скачивание, проверка контрольных сумм и биты исполнения остаются в
//! `mkdsc/tools.py`: Python-бэкенд стартует первым и занимается бутстрапом.
//! Rust'у нужно только найти уже лежащий на диске бинарь — в том же порядке,
//! что и Python, иначе два процесса могли бы запускать разные adb.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::AppHandle;

use crate::paths;

/// Найденный путь к adb.
///
/// Кэшируется только успех. Запомнить `None` нельзя: при первом запуске
/// Python ещё качает platform-tools, и отрицательный результат навсегда
/// оставил бы приложение без списка устройств до перезапуска.
static ADB_CACHE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Найденный путь к scrcpy — см. [`ADB_CACHE`].
static SCRCPY_CACHE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Путь к adb или `None`, если бинарь ещё не появился на диске.
pub fn adb_path(app: &AppHandle) -> Option<PathBuf> {
    cached_or_locate(&ADB_CACHE, || locate_adb(app))
}

/// Путь к scrcpy или `None`, если бинарь ещё не появился на диске.
pub fn scrcpy_path(app: &AppHandle) -> Option<PathBuf> {
    cached_or_locate(&SCRCPY_CACHE, || locate_scrcpy(app))
}

/// Отдаёт закэшированный путь или ищет заново и запоминает успех.
fn cached_or_locate(
    cache: &Mutex<Option<PathBuf>>,
    locate: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    if let Ok(cached) = cache.lock() {
        if let Some(path) = cached.as_ref() {
            // Кэш перепроверяется: между опросами бинарь могли снести.
            if path.is_file() {
                return Some(path.clone());
            }
        }
    }

    let found = locate()?;
    if let Ok(mut cache) = cache.lock() {
        *cache = Some(found.clone());
    }
    Some(found)
}

fn adb_file_name() -> &'static str {
    if cfg!(windows) { "adb.exe" } else { "adb" }
}

fn scrcpy_file_name() -> &'static str {
    if cfg!(windows) { "scrcpy.exe" } else { "scrcpy" }
}

fn locate_adb(app: &AppHandle) -> Option<PathBuf> {
    let name = adb_file_name();

    // 1. Явный путь из окружения — как `MKDSC_ADB_PATH` в `mkdsc/tools.py`.
    if let Some(value) = std::env::var_os("MKDSC_ADB_PATH") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }

    // 2-3. Точечные кандидаты. Рекурсивный обход из Python здесь не
    // повторяется: там он уже стоил прохода по `node_modules`, а распаковщик
    // кладёт adb во вполне определённое место.
    let downloads = paths::downloads_dir(app);
    let base = paths::base_dir(app);
    let candidates = [
        downloads.join("platform-tools").join(name),
        downloads.join(name),
        base.join("bin").join(name),
        base.join("downloads").join("platform-tools").join(name),
        base.join("downloads").join(name),
    ];
    if let Some(found) = candidates.into_iter().find(|path| path.is_file()) {
        return Some(found);
    }

    // 4. PATH.
    find_in_path(name)
}

fn locate_scrcpy(app: &AppHandle) -> Option<PathBuf> {
    let name = scrcpy_file_name();

    // 1. Явный путь из окружения — как `MKDSC_SCRCPY_PATH` в `mkdsc/tools.py`.
    if let Some(value) = std::env::var_os("MKDSC_SCRCPY_PATH") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }

    // 2. Каталоги распаковки. Точечными кандидатами, как у adb, не обойтись:
    // архив разворачивается в каталог с версией в имени
    // (`scrcpy-win64-v3.3.4/scrcpy.exe`), и она меняется от релиза к релизу.
    let downloads = paths::downloads_dir(app);
    let base = paths::base_dir(app);
    let roots = [
        downloads,
        base.join("downloads"),
        base.join("bin"),
        base.to_path_buf(),
    ];
    for root in roots {
        if let Some(found) = scan_for(&root, name, SCAN_DEPTH) {
            return Some(found);
        }
    }

    // 3. PATH.
    find_in_path(name)
}

/// На сколько уровней вглубь заглядывать в поисках бинаря.
///
/// Python берёт пять (`_MAX_SEARCH_DEPTH`), но ему это нужно ради каталогов, в
/// которые он сам же и распаковывает архивы вслепую. Здесь хватает двух:
/// глубже `downloads/<каталог релиза>/` ни adb, ни scrcpy не лежат, а каждый
/// лишний уровень — это обход дерева репозитория на старте.
const SCAN_DEPTH: usize = 2;

/// Каталоги, в которые бессмысленно спускаться, — как `_IGNORED_SEARCH_DIRS`.
const IGNORED_DIRS: [&str; 6] = [
    "node_modules",
    "target",
    "dist",
    "dist-tauri",
    "site-packages",
    "__pycache__",
];

/// Ищет файл в каталоге не глубже `depth` уровней.
fn scan_for(root: &std::path::Path, name: &str, depth: usize) -> Option<PathBuf> {
    let direct = root.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }

    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skip = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| name.starts_with('.') || IGNORED_DIRS.contains(&name));
        if skip {
            continue;
        }
        if let Some(found) = scan_for(&path, name, depth - 1) {
            return Some(found);
        }
    }

    None
}

/// Минимальный аналог `shutil.which` — ради одного вызова тащить зависимость
/// незачем.
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}
