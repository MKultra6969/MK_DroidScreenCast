//! Экспорт логов в zip — порт `_create_logs_zip` и `POST /api/logs/export`.
//!
//! Архив собирается для отправки в баг-репорт, поэтому в нём не только логи:
//! конфиг и версии adb/scrcpy объясняют половину вопросов, которые иначе
//! пришлось бы задавать.
//!
//! Архив сразу пишется в выбранный пользователем каталог. Отдельного
//! «скачивания» (`GET /api/logs/download` в HTTP-версии) больше нет: файл
//! потоком через IPC не отдать, да и незачем — он уже на диске.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::AppHandle;
use zip::write::SimpleFileOptions;

use crate::error::ApiError;
use crate::{config, devices, paths, tools};

/// Таймаут опроса версии инструмента.
///
/// `adb version` поднимает демона, если тот не запущен, — отсюда запас. Сбой
/// или таймаут не срывают экспорт: в архив уедет то, что успели получить.
const VERSION_TIMEOUT: Duration = Duration::from_secs(20);

/// Собирает архив с логами в указанном каталоге.
///
/// Возвращает `{success, path, filename}`. Ошибки: 400 — каталог не задан, не
/// создаётся или оказался файлом; 500 — архив не записался.
pub async fn export(app: &AppHandle, directory: Option<&str>) -> Result<Value, ApiError> {
    let Some(directory) = directory.filter(|value| !value.is_empty()) else {
        return Err(ApiError::new(400, "Directory required"));
    };

    let target_dir = paths::expand_user(directory);
    std::fs::create_dir_all(&target_dir).map_err(|err| ApiError::new(400, err.to_string()))?;
    if !target_dir.is_dir() {
        return Err(ApiError::new(400, "Target path is not a directory"));
    }

    // Имя со временем: экспорт делают несколько раз подряд, и второй архив не
    // должен затирать первый.
    let file_name = format!(
        "logs_{}.zip",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let zip_path = target_dir.join(&file_name);

    let config = config::load(app)?;
    let logs_dir = paths::logs_dir(app, &config);
    let adb_version = tool_version(tools::adb_path(app).as_deref(), &["version"]).await;
    let scrcpy_version = tool_version(tools::scrcpy_path(app).as_deref(), &["--version"]).await;

    create_zip(
        &zip_path,
        &logs_dir,
        &paths::config_path(app),
        &adb_version,
        &scrcpy_version,
    )
    .map_err(|err| ApiError::internal(err.to_string()))?;

    Ok(json!({
        "success": true,
        "path": zip_path.to_string_lossy(),
        "filename": file_name,
    }))
}

/// Пишет архив: логи, конфиг и три текстовых файла с версиями.
fn create_zip(
    zip_path: &Path,
    logs_dir: &Path,
    config_path: &Path,
    adb_version: &str,
    scrcpy_version: &str,
) -> std::io::Result<()> {
    let file = std::fs::File::create(zip_path)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for log_file in log_files(logs_dir) {
        let Some(name) = log_file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // Лог мог уехать из-под ног (ротация, чужая уборка) — пропускаем его,
        // а не роняем весь экспорт: остальные логи всё ещё полезны.
        let Ok(contents) = std::fs::read(&log_file) else {
            continue;
        };
        archive.start_file(format!("logs/{name}"), options)?;
        archive.write_all(&contents)?;
    }

    if let Ok(contents) = std::fs::read(config_path) {
        archive.start_file("config.json", options)?;
        archive.write_all(&contents)?;
    }

    archive.start_file("version.txt", options)?;
    archive.write_all(env!("CARGO_PKG_VERSION").as_bytes())?;
    archive.start_file("adb_version.txt", options)?;
    archive.write_all(adb_version.as_bytes())?;
    archive.start_file("scrcpy_version.txt", options)?;
    archive.write_all(scrcpy_version.as_bytes())?;

    archive.finish()?;
    Ok(())
}

/// Файлы `*.log` в каталоге логов — аналог `logs_dir.glob("*.log")`.
///
/// Отсортированы по имени: `glob` порядок не гарантирует, а разный порядок
/// записей в архиве мешает сравнивать два экспорта между собой.
fn log_files(logs_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(logs_dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    files.sort();
    files
}

/// Вывод `<tool> <args>` или `not available` — как `_tool_version`.
///
/// `devices::run_adb` здесь просто «запусти бинарь с таймаутом»: своего
/// про adb в ней ничего нет.
async fn tool_version(tool: Option<&Path>, args: &[&str]) -> String {
    let Some(tool) = tool else {
        return "not available".to_string();
    };

    match devices::run_adb(tool, args, VERSION_TIMEOUT).await {
        Ok(output) => output.stdout,
        Err(error) => error.detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mkdsc-logs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("каталог создан");
        dir
    }

    #[test]
    fn archive_holds_logs_config_and_versions() {
        let root = temp_dir("archive");
        let logs_dir = root.join("logs");
        std::fs::create_dir_all(&logs_dir).expect("каталог логов");
        std::fs::write(logs_dir.join("web.log"), "hello").expect("лог");
        std::fs::write(logs_dir.join("cli.log"), "world").expect("лог");
        // Не `.log` — в архив не попадает.
        std::fs::write(logs_dir.join("notes.txt"), "skip").expect("файл");

        let config_path = root.join("config.json");
        std::fs::write(&config_path, "{\"language\": \"ru\"}").expect("конфиг");

        let zip_path = root.join("logs.zip");
        create_zip(&zip_path, &logs_dir, &config_path, "adb 35", "scrcpy 3.3").expect("архив");

        let file = std::fs::File::open(&zip_path).expect("архив открылся");
        let mut archive = zip::ZipArchive::new(file).expect("это zip");
        let names: Vec<String> = archive.file_names().map(str::to_string).collect();

        for expected in [
            "logs/web.log",
            "logs/cli.log",
            "config.json",
            "version.txt",
            "adb_version.txt",
            "scrcpy_version.txt",
        ] {
            assert!(names.contains(&expected.to_string()), "нет {expected}");
        }
        assert!(!names.iter().any(|name| name.ends_with(".txt") && name.contains("notes")));

        let mut entry = archive.by_name("adb_version.txt").expect("запись на месте");
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut entry, &mut contents).expect("прочиталось");
        assert_eq!(contents, "adb 35");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Ни логов, ни конфига — архив всё равно собирается: версии в нём уже
    /// что-то объясняют, а падать на пустом каталоге незачем.
    #[test]
    fn archive_survives_missing_logs_and_config() {
        let root = temp_dir("empty");
        let zip_path = root.join("logs.zip");

        create_zip(
            &zip_path,
            &root.join("nope"),
            &root.join("nope.json"),
            "not available",
            "not available",
        )
        .expect("архив");

        let file = std::fs::File::open(&zip_path).expect("архив открылся");
        let archive = zip::ZipArchive::new(file).expect("это zip");
        assert_eq!(archive.len(), 3);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_log_files_are_collected_and_sorted() {
        let root = temp_dir("glob");
        for name in ["b.log", "a.log", "c.txt"] {
            std::fs::write(root.join(name), "x").expect("файл");
        }
        std::fs::create_dir_all(root.join("nested.log")).expect("каталог");

        let files: Vec<String> = log_files(&root)
            .iter()
            .filter_map(|path| path.file_name()?.to_str().map(str::to_string))
            .collect();
        assert_eq!(files, ["a.log", "b.log"]);
        // Несуществующий каталог — пустой список, а не ошибка.
        assert!(log_files(&root.join("missing")).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
