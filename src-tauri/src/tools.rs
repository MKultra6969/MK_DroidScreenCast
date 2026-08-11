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

/// Путь к adb или `None`, если бинарь ещё не появился на диске.
pub fn adb_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(cached) = ADB_CACHE.lock() {
        if let Some(path) = cached.as_ref() {
            // Кэш перепроверяется: между опросами бинарь могли снести.
            if path.is_file() {
                return Some(path.clone());
            }
        }
    }

    let found = locate_adb(app)?;
    if let Ok(mut cache) = ADB_CACHE.lock() {
        *cache = Some(found.clone());
    }
    Some(found)
}

fn adb_file_name() -> &'static str {
    if cfg!(windows) { "adb.exe" } else { "adb" }
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

/// Минимальный аналог `shutil.which` — ради одного вызова тащить зависимость
/// незачем.
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}
