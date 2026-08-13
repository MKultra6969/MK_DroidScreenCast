//! Пути приложения — Rust-половина `mkdsc/paths.py`.
//!
//! Пока Python-бэкенд жив, оба процесса работают с одним и тем же
//! `config.json` и одним и тем же `downloads/`, поэтому расходиться в
//! вычислении путей нельзя. Согласованность держится на том, что `main.rs`
//! передаёт бэкенду вычисленные здесь значения через `MKDSC_BASE_DIR` и
//! `MKDSC_DATA_DIR`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Корень установки: рядом лежат `tauri_backend.py`, `mkdsc/` или `bin/`.
pub fn base_dir(app: &AppHandle) -> &'static Path {
    BASE_DIR.get_or_init(|| resolve_base_dir(app)).as_path()
}

/// Каталог пользовательских данных: `config.json`, `downloads/`, `logs/`.
pub fn data_dir(app: &AppHandle) -> &'static Path {
    DATA_DIR.get_or_init(|| resolve_data_dir(app)).as_path()
}

/// `DATA_DIR/downloads` — сюда Python распаковывает adb и scrcpy.
pub fn downloads_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("downloads")
}

/// `DATA_DIR/config.json` — тот же файл, что читает Python.
pub fn config_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("config.json")
}

/// Каталог для записей по умолчанию — `get_recordings_dir` в `mkdsc/paths.py`.
///
/// Если пользователь задал `downloads.base_dir`, записи уезжают в его
/// подкаталог `video`, иначе — в `DATA_DIR/recordings`. Расходиться с Python
/// нельзя: пока живы обе панели, они должны показывать одни и те же файлы.
pub fn recordings_dir(app: &AppHandle, config: &serde_json::Map<String, serde_json::Value>) -> PathBuf {
    match downloads_base_dir(config) {
        Some(base) if base != data_dir(app) => base.join("video"),
        _ => data_dir(app).join("recordings"),
    }
}

/// Каталог логов — `get_logs_dir` в `mkdsc/paths.py`.
///
/// Логи пишет Python, а забирает их отсюда экспорт: разойтись нельзя, иначе
/// архив уедет пустым.
pub fn logs_dir(app: &AppHandle, config: &serde_json::Map<String, serde_json::Value>) -> PathBuf {
    match downloads_base_dir(config) {
        Some(base) if base != data_dir(app) => base.join("logs"),
        _ => data_dir(app).join("logs"),
    }
}

/// Каталог скриншотов — `get_screenshots_dir` в `mkdsc/paths.py`.
pub fn screenshots_dir(
    app: &AppHandle,
    config: &serde_json::Map<String, serde_json::Value>,
) -> PathBuf {
    match downloads_base_dir(config) {
        Some(base) if base != data_dir(app) => base.join("screenshots"),
        _ => data_dir(app).join("screenshots"),
    }
}

/// Куда складывать скачанное с устройства — `_resolve_download_dir`.
///
/// Обратите внимание на несимметричность с логами и скриншотами: при заданном
/// `downloads.base_dir` файлы кладутся прямо в него, без подкаталога, а при
/// незаданном — в `DATA_DIR/downloads`. Так это работает в Python, и менять
/// нельзя: пользователь ищет файлы там, куда их клал прошлый релиз.
pub fn download_dir(app: &AppHandle, config: &serde_json::Map<String, serde_json::Value>) -> PathBuf {
    match downloads_base_dir(config) {
        Some(base) if base != data_dir(app) => base,
        _ => data_dir(app).join("downloads"),
    }
}

/// `downloads.base_dir` из конфига, если он задан непустой строкой.
fn downloads_base_dir(config: &serde_json::Map<String, serde_json::Value>) -> Option<PathBuf> {
    let base = config
        .get("downloads")?
        .as_object()?
        .get("base_dir")?
        .as_str()?
        .trim();
    (!base.is_empty()).then(|| expand_user(base))
}

/// Раскрывает ведущую тильду — аналог `Path.expanduser()`.
///
/// Пользователь вправе написать в конфиге `~/Videos`, и без раскрытия каталог
/// с именем `~` создался бы прямо в рабочем каталоге приложения.
pub fn expand_user(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix('~') else {
        return PathBuf::from(path);
    };
    // Раскрывается только «~» и «~/...»: `~user` Python на Windows тоже не
    // разбирает, а гадать за пользователя тут не из чего.
    if !rest.is_empty() && !rest.starts_with(['/', '\\']) {
        return PathBuf::from(path);
    }

    let Some(home) = home_dir() else {
        return PathBuf::from(path);
    };
    let rest = rest.trim_start_matches(['/', '\\']);
    if rest.is_empty() {
        home
    } else {
        home.join(rest)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
}

/// `BASE_DIR/devices.json` — формат до появления `config.json`.
///
/// Читается только миграцией, и только пока список устройств пуст. Путь идёт
/// от `BASE_DIR`, а не от `DATA_DIR`: так его считает `mkdsc/paths.py`, и
/// разойтись здесь означало бы потерять устройства у тех, кто обновляется со
/// старой версии.
pub fn legacy_devices_path(app: &AppHandle) -> PathBuf {
    base_dir(app).join("devices.json")
}

fn resolve_base_dir(app: &AppHandle) -> PathBuf {
    if let Some(explicit) = std::env::var_os("MKDSC_BASE_DIR") {
        return PathBuf::from(explicit);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.clone());
        candidates.push(dir.join("app"));
        candidates.push(dir.join("_up_"));
    }
    if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.clone());
        if let Some(parent) = dir.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_dir.parent() {
        candidates.push(parent.to_path_buf());
    }

    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
    let backend_name = format!("mkdsc-backend{exe_suffix}");

    for base in candidates {
        if base.join("tauri_backend.py").exists()
            || base.join("bin").join(&backend_name).exists()
            || base.join("mkdsc").exists()
        {
            return base;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn resolve_data_dir(app: &AppHandle) -> PathBuf {
    // `MKDSC_DATA_DIR` идёт первым — как в `mkdsc/paths.py`. Раньше здесь
    // переменная не проверялась, и запуск с внешним каталогом данных развёл
    // бы Rust и Python по разным `config.json`.
    if let Some(explicit) = std::env::var_os("MKDSC_DATA_DIR") {
        return PathBuf::from(explicit);
    }

    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| base_dir(app).to_path_buf())
}
