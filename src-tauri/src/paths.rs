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

/// `DATA_DIR/config.json` — тот же файл, что читает и пишет Python.
pub fn config_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("config.json")
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
