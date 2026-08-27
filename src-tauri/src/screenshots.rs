//! Галерея скриншотов — порт `mkdsc/web/gallery.py`.
//!
//! Хранилище — каталог с png и `metadata.json` рядом: id, имя файла, подпись,
//! время и серийник устройства. Формат файла менять нельзя: его пишут
//! предыдущие версии, и галерея должна открывать снятое ими.
//!
//! Одно поле в ответе добавлено сверх REST — `path`, абсолютный путь к файлу
//! на диске. По HTTP картинка отдавалась эндпоинтом `GET /api/screenshots/{id}`,
//! но без сервера URL взять неоткуда: фронтенд превращает этот путь в
//! `asset://` через `convertFileSrc`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::devices;
use crate::error::ApiError;

/// `screencap` и `pull` — `timeout=30` в Python.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);

/// Уборка файла с устройства — `timeout=10`.
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Размер страницы галереи по умолчанию и его потолок.
const DEFAULT_PAGE_SIZE: usize = 24;
const MAX_PAGE_SIZE: usize = 200;

/// Имя файла с метаданными.
const METADATA_FILE: &str = "metadata.json";

/// Создаёт каталог и пустой `metadata.json` — `ensure_dir`.
fn ensure_dir(dir: &Path) -> Result<(), ApiError> {
    std::fs::create_dir_all(dir).map_err(|err| ApiError::internal(err.to_string()))?;
    let metadata = dir.join(METADATA_FILE);
    if !metadata.exists() {
        std::fs::write(&metadata, "[]").map_err(|err| ApiError::internal(err.to_string()))?;
    }
    Ok(())
}

/// Читает `metadata.json` — `load_metadata`.
///
/// Битый файл переименовывается в `.json.corrupt`, а не затирается молча при
/// первом же сохранении вместе со всеми подписями.
fn load_metadata(dir: &Path) -> Result<Vec<Value>, ApiError> {
    ensure_dir(dir)?;
    let path = dir.join(METADATA_FILE);

    let parsed = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());

    let Some(value) = parsed else {
        let _ = std::fs::rename(&path, dir.join("metadata.json.corrupt"));
        return Ok(Vec::new());
    };

    let Value::Array(items) = value else {
        return Ok(Vec::new());
    };

    // Записи без id бесполезны: по нему идёт вся адресация.
    Ok(items
        .into_iter()
        .filter(|entry| entry.get("id").is_some_and(Value::is_string))
        .collect())
}

/// Пишет `metadata.json` в том же виде, в каком его пишет Python.
fn save_metadata(dir: &Path, entries: &[Value]) -> Result<(), ApiError> {
    ensure_dir(dir)?;
    let text = serde_json::to_string_pretty(&Value::Array(entries.to_vec()))
        .map_err(|err| ApiError::internal(err.to_string()))?;
    std::fs::write(dir.join(METADATA_FILE), text)
        .map_err(|err| ApiError::internal(err.to_string()))
}

fn entry_str<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// Копия записи с добавленным путём к файлу — для `convertFileSrc`.
fn with_path(entry: &Value, dir: &Path) -> Value {
    let mut copy = entry.clone();
    if let Some(object) = copy.as_object_mut() {
        let file = dir.join(entry_str(entry, "filename"));
        object.insert("path".to_string(), json!(file.to_string_lossy()));
    }
    copy
}

/// Зеркалит `GET /api/screenshots` — страница галереи.
///
/// Записи, чьи файлы исчезли с диска, вычищаются: пользователь мог удалить
/// картинку мимо приложения, а битая плитка в галерее выглядит как поломка.
pub fn list(dir: &Path, page: usize, page_size: usize) -> Result<Value, ApiError> {
    let metadata = load_metadata(dir)?;

    let mut valid: Vec<Value> = metadata
        .iter()
        .filter(|entry| dir.join(entry_str(entry, "filename")).exists())
        .cloned()
        .collect();

    if valid.len() != metadata.len() {
        save_metadata(dir, &valid)?;
    }

    // Новые сверху. Ключ — время создания, а при его отсутствии id: так же
    // подставляет запасной ключ Python.
    valid.sort_by(|left, right| sort_key(right).cmp(sort_key(left)));

    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
    let total_count = valid.len();
    let total_pages = total_count.div_ceil(page_size).max(1);
    let page = page.max(1).min(total_pages);
    let start = (page - 1) * page_size;
    let items: Vec<Value> = valid
        .iter()
        .skip(start)
        .take(page_size)
        .map(|entry| with_path(entry, dir))
        .collect();

    Ok(json!({
        "screenshots": items,
        "count": total_count,
        "page_count": items.len(),
        "total_count": total_count,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    }))
}

fn sort_key(entry: &Value) -> &str {
    let created = entry_str(entry, "created_at");
    if created.is_empty() {
        entry_str(entry, "id")
    } else {
        created
    }
}

/// Зеркалит `POST /api/screenshots/take`.
///
/// Снимает экран на устройстве, забирает файл и удаляет его с устройства.
/// Ошибки — 500 с текстом от adb: интерфейс показывает его как есть.
pub async fn take(
    adb: &Path,
    dir: &Path,
    serial: Option<&str>,
    caption: &str,
) -> Result<Value, ApiError> {
    ensure_dir(dir)?;

    // Секундной точности не хватало: два скриншота в одну секунду получали
    // одинаковый id и одно имя файла — второй затирал первый.
    let id = uuid::Uuid::new_v4().simple().to_string();
    let filename = format!(
        "screenshot_{}_{}.png",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        &id[..8]
    );
    let device_path = format!("/sdcard/{filename}");
    let local_path = dir.join(&filename);

    // `exec-out` отдаёт png прямо в stdout: один запуск adb вместо трёх и ни
    // одного файла на телефоне. Прежний путь снимал в /sdcard, тянул файл
    // через `pull` и потом удалял — три подключения к устройству на каждый
    // снимок, отсюда и заметная задержка.
    let quick = run_bytes(adb, serial, &["exec-out", "screencap", "-p"], CAPTURE_TIMEOUT).await?;
    if quick.timed_out {
        return Err(ApiError::internal("Screenshot operation timed out"));
    }

    if quick.success() && is_png(&quick.stdout) {
        std::fs::write(&local_path, &quick.stdout)
            .map_err(|err| ApiError::internal(err.to_string()))?;
    } else {
        // `exec-out` понимает не всякая прошивка, а часть отдаёт по нему пустой
        // или испорченный поток — тогда работаем по-старому.
        capture_through_device(adb, serial, &device_path, &local_path).await?;
    }

    let entry = json!({
        "id": id,
        "filename": filename,
        "caption": caption,
        "created_at": crate::api::now_iso(),
        "size_bytes": std::fs::metadata(&local_path).map(|meta| meta.len()).unwrap_or(0),
        "device_serial": serial,
    });

    let mut metadata = load_metadata(dir)?;
    metadata.push(entry.clone());
    save_metadata(dir, &metadata)?;

    Ok(json!({"success": true, "screenshot": with_path(&entry, dir)}))
}

/// Зеркалит `POST /api/screenshots/{id}/save` — копия в локальную папку.
pub fn save(dir: &Path, id: &str, target_dir: &Path) -> Result<Value, ApiError> {
    let metadata = load_metadata(dir)?;
    let entry = find(&metadata, id)?;

    let source = dir.join(entry_str(entry, "filename"));
    if !source.exists() {
        return Err(ApiError::new(404, "Screenshot file not found"));
    }

    std::fs::create_dir_all(target_dir).map_err(|err| ApiError::new(400, err.to_string()))?;
    let target = target_dir.join(entry_str(entry, "filename"));
    std::fs::copy(&source, &target).map_err(|err| ApiError::new(400, err.to_string()))?;

    Ok(json!({"success": true, "path": target.to_string_lossy()}))
}

/// Зеркалит `PUT /api/screenshots/{id}/caption`.
pub fn set_caption(dir: &Path, id: &str, caption: &str) -> Result<Value, ApiError> {
    let mut metadata = load_metadata(dir)?;

    let Some(entry) = metadata
        .iter_mut()
        .find(|entry| entry_str(entry, "id") == id)
    else {
        return Err(ApiError::new(404, "Screenshot not found"));
    };

    if let Some(object) = entry.as_object_mut() {
        object.insert("caption".to_string(), json!(caption));
    }
    let updated = entry.clone();
    save_metadata(dir, &metadata)?;

    Ok(json!({"success": true, "screenshot": with_path(&updated, dir)}))
}

/// Зеркалит `DELETE /api/screenshots/{id}`.
pub fn delete(dir: &Path, id: &str) -> Result<Value, ApiError> {
    let metadata = load_metadata(dir)?;
    let entry = find(&metadata, id)?;

    // Файла может уже не быть — это не ошибка, запись всё равно уходит.
    let _ = std::fs::remove_file(dir.join(entry_str(entry, "filename")));

    let remaining: Vec<Value> = metadata
        .iter()
        .filter(|entry| entry_str(entry, "id") != id)
        .cloned()
        .collect();
    save_metadata(dir, &remaining)?;

    Ok(json!({"success": true}))
}

/// Зеркалит `DELETE /api/screenshots` — удаление списком.
///
/// Считаются только найденные записи: неизвестный id не ошибка, но и в
/// `deleted_count` не попадает.
pub fn delete_many(dir: &Path, ids: &[String]) -> Result<Value, ApiError> {
    let metadata = load_metadata(dir)?;
    let mut deleted = 0;

    for id in ids {
        if let Some(entry) = metadata.iter().find(|entry| entry_str(entry, "id") == id) {
            let _ = std::fs::remove_file(dir.join(entry_str(entry, "filename")));
            deleted += 1;
        }
    }

    let remaining: Vec<Value> = metadata
        .iter()
        .filter(|entry| !ids.iter().any(|id| id == entry_str(entry, "id")))
        .cloned()
        .collect();
    save_metadata(dir, &remaining)?;

    Ok(json!({"success": true, "deleted_count": deleted}))
}

fn find<'a>(metadata: &'a [Value], id: &str) -> Result<&'a Value, ApiError> {
    metadata
        .iter()
        .find(|entry| entry_str(entry, "id") == id)
        .ok_or_else(|| ApiError::new(404, "Screenshot not found"))
}

/// png начинается с восьмибайтовой сигнатуры — по ней и отличаем настоящий
/// снимок от сообщения об ошибке, которое прошивка могла выдать в stdout.
fn is_png(data: &[u8]) -> bool {
    data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
}

/// Запасной путь: снять в файл на устройстве, забрать его и убрать за собой.
async fn capture_through_device(
    adb: &Path,
    serial: Option<&str>,
    device_path: &str,
    local_path: &Path,
) -> Result<(), ApiError> {
    let capture = run(
        adb,
        serial,
        &["shell", "screencap", "-p", device_path],
        CAPTURE_TIMEOUT,
    )
    .await?;
    if capture.timed_out {
        return Err(ApiError::internal("Screenshot operation timed out"));
    }
    if !capture.success() {
        return Err(ApiError::internal(format!(
            "Failed to capture screenshot: {}",
            capture.stderr
        )));
    }

    let local = local_path.to_string_lossy().into_owned();
    let pull = run(adb, serial, &["pull", device_path, &local], CAPTURE_TIMEOUT).await?;
    if pull.timed_out {
        return Err(ApiError::internal("Screenshot operation timed out"));
    }
    if !pull.success() || !local_path.exists() {
        return Err(ApiError::internal(format!(
            "Failed to pull screenshot: {}",
            pull.stderr
        )));
    }

    // Файл на устройстве больше не нужен; неудача уборки не повод отдавать
    // ошибку — скриншот уже на диске.
    let _ = run(adb, serial, &["shell", "rm", device_path], CLEANUP_TIMEOUT).await;
    Ok(())
}

/// То же, что `run`, но stdout не декодируется — для `exec-out`.
async fn run_bytes(
    adb: &Path,
    serial: Option<&str>,
    args: &[&str],
    timeout: Duration,
) -> Result<devices::AdbBytes, ApiError> {
    let mut argv: Vec<&str> = Vec::new();
    if let Some(serial) = serial {
        argv.push("-s");
        argv.push(serial);
    }
    argv.extend_from_slice(args);
    devices::run_adb_bytes(adb, &argv, timeout).await
}

async fn run(
    adb: &Path,
    serial: Option<&str>,
    args: &[&str],
    timeout: Duration,
) -> Result<devices::AdbOutput, ApiError> {
    let mut argv: Vec<&str> = Vec::new();
    if let Some(serial) = serial {
        argv.push("-s");
        argv.push(serial);
    }
    argv.extend_from_slice(args);
    devices::run_adb(adb, &argv, timeout).await
}

/// Разбирает `page`/`page_size` галереи из строки запроса.
pub fn paging(query: &std::collections::HashMap<String, String>) -> (usize, usize) {
    let page = query
        .get("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let page_size = query
        .get("page_size")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    (page, page_size)
}

/// Список id из тела запроса на групповое удаление.
///
/// FastAPI принимает здесь голый список, поэтому его же принимает и роутер;
/// объект `{"ids": [...]}` тоже разбирается — на случай, если фронтенд когда-то
/// станет слать его.
pub fn ids_of(body: Option<Value>) -> Vec<String> {
    let items = match body {
        Some(Value::Array(items)) => items,
        Some(Value::Object(map)) => match map.get("ids") {
            Some(Value::Array(items)) => items.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };

    items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

/// Открывает каталог скриншотов asset-протоколу.
///
/// Область видимости протокола в `tauri.conf.json` пуста намеренно: каталог
/// зависит от `downloads.base_dir`, то есть известен только в рантайме.
/// Вызывается перед каждой отдачей списка — пользователь мог сменить папку в
/// настройках, и разрешение, выданное на старте, указывало бы не туда.
pub fn allow_asset_access(app: &tauri::AppHandle, dir: &Path) {
    use tauri::Manager;

    // Ошибка означает только то, что путь не разобрался как шаблон; картинки
    // тогда не покажутся, но список отдать всё равно нужно.
    let _ = app.asset_protocol_scope().allow_directory(dir, false);
}

/// Каталог, куда `save` кладёт файл без явного назначения.
pub fn default_save_dir(
    app: &tauri::AppHandle,
    config: &Map<String, Value>,
    destination: Option<&str>,
) -> PathBuf {
    match destination {
        Some(directory) => crate::paths::expand_user(directory),
        None => crate::paths::download_dir(app, config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mkdsc-shots-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("каталог создан");
        dir
    }

    fn entry(id: &str, filename: &str, created_at: &str) -> Value {
        json!({
            "id": id,
            "filename": filename,
            "caption": "",
            "created_at": created_at,
            "size_bytes": 1,
            "device_serial": Value::Null,
        })
    }

    fn seed(dir: &Path, entries: &[Value]) {
        for item in entries {
            std::fs::write(dir.join(entry_str(item, "filename")), "png").expect("файл");
        }
        save_metadata(dir, entries).expect("метаданные");
    }

    #[test]
    fn list_sorts_newest_first_and_paginates() {
        let dir = temp_dir("list");
        seed(
            &dir,
            &[
                entry("a", "a.png", "2026-08-01T10:00:00"),
                entry("b", "b.png", "2026-08-03T10:00:00"),
                entry("c", "c.png", "2026-08-02T10:00:00"),
            ],
        );

        let page = list(&dir, 1, 2).expect("список");
        assert_eq!(page["total_count"], json!(3));
        assert_eq!(page["count"], json!(3));
        assert_eq!(page["page_count"], json!(2));
        assert_eq!(page["total_pages"], json!(2));
        assert_eq!(page["screenshots"][0]["id"], json!("b"));
        assert_eq!(page["screenshots"][1]["id"], json!("c"));

        let second = list(&dir, 2, 2).expect("список");
        assert_eq!(second["screenshots"][0]["id"], json!("a"));
        assert_eq!(second["page"], json!(2));

        // Страница за пределами прижимается к последней.
        assert_eq!(list(&dir, 99, 2).expect("список")["page"], json!(2));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Путь к файлу уезжает в ответ, но не в `metadata.json`: иначе он утёк бы
    /// в файл, который читает и Python, и переезд каталога сломал бы записи.
    #[test]
    fn list_adds_path_without_persisting_it() {
        let dir = temp_dir("path");
        seed(&dir, &[entry("a", "a.png", "2026-08-01T10:00:00")]);

        let page = list(&dir, 1, 24).expect("список");
        let path = page["screenshots"][0]["path"].as_str().expect("путь");
        assert!(path.ends_with("a.png"), "путь: {path}");
        assert_eq!(PathBuf::from(path), dir.join("a.png"));

        let raw = std::fs::read_to_string(dir.join(METADATA_FILE)).expect("метаданные");
        assert!(!raw.contains("\"path\""), "путь просочился в metadata.json");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Запись без файла на диске исчезает из галереи и из метаданных.
    #[test]
    fn list_drops_entries_whose_file_is_gone() {
        let dir = temp_dir("stale");
        seed(
            &dir,
            &[
                entry("a", "a.png", "2026-08-01T10:00:00"),
                entry("b", "b.png", "2026-08-02T10:00:00"),
            ],
        );
        std::fs::remove_file(dir.join("a.png")).expect("файл удалён");

        let page = list(&dir, 1, 24).expect("список");
        assert_eq!(page["total_count"], json!(1));
        assert_eq!(page["screenshots"][0]["id"], json!("b"));

        assert_eq!(load_metadata(&dir).expect("метаданные").len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_metadata_is_kept_aside() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).expect("каталог");
        std::fs::write(dir.join(METADATA_FILE), "{ not json").expect("битый файл");

        assert!(load_metadata(&dir).expect("список").is_empty());
        assert!(
            dir.join("metadata.json.corrupt").exists(),
            "битый файл должен сохраниться рядом, а не пропасть"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn caption_delete_and_bulk_delete() {
        let dir = temp_dir("mutate");
        seed(
            &dir,
            &[
                entry("a", "a.png", "2026-08-01T10:00:00"),
                entry("b", "b.png", "2026-08-02T10:00:00"),
                entry("c", "c.png", "2026-08-03T10:00:00"),
            ],
        );

        let updated = set_caption(&dir, "a", "подпись").expect("подпись");
        assert_eq!(updated["screenshot"]["caption"], json!("подпись"));
        assert_eq!(
            set_caption(&dir, "nope", "x").unwrap_err().status,
            404
        );

        assert_eq!(delete(&dir, "a").expect("удаление")["success"], json!(true));
        assert!(!dir.join("a.png").exists());
        assert_eq!(delete(&dir, "a").unwrap_err().status, 404);

        let bulk = delete_many(&dir, &["b".to_string(), "nope".to_string()]).expect("групповое");
        assert_eq!(bulk["deleted_count"], json!(1));
        assert!(!dir.join("b.png").exists());

        let left = load_metadata(&dir).expect("метаданные");
        assert_eq!(left.len(), 1);
        assert_eq!(entry_str(&left[0], "id"), "c");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_copies_the_file_and_reports_the_path() {
        let dir = temp_dir("save");
        seed(&dir, &[entry("a", "a.png", "2026-08-01T10:00:00")]);
        let target = dir.join("out");

        let saved = save(&dir, "a", &target).expect("сохранение");
        assert_eq!(saved["success"], json!(true));
        assert!(target.join("a.png").exists());
        assert_eq!(
            PathBuf::from(saved["path"].as_str().expect("путь")),
            target.join("a.png")
        );

        assert_eq!(save(&dir, "nope", &target).unwrap_err().status, 404);

        // Записи без файла на диске — тоже 404, а не пустой файл в папке.
        std::fs::remove_file(dir.join("a.png")).expect("удалён");
        assert_eq!(save(&dir, "a", &target).unwrap_err().detail, "Screenshot file not found");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_are_read_from_a_bare_list_or_an_object() {
        assert_eq!(ids_of(Some(json!(["a", "b"]))), vec!["a", "b"]);
        assert_eq!(ids_of(Some(json!({"ids": ["a"]}))), vec!["a"]);
        assert!(ids_of(Some(json!({"ids": "a"}))).is_empty());
        assert!(ids_of(Some(json!("a"))).is_empty());
        assert!(ids_of(None).is_empty());
        // Нестроковые элементы пропускаются, а не роняют разбор.
        assert_eq!(ids_of(Some(json!(["a", 1, null]))), vec!["a"]);
    }
}
