//! Файловый менеджер поверх adb — порт `mkdsc/web/file_manager.py`.
//!
//! Здесь три вещи, которые важнее остального кода:
//!
//! 1. **Квотирование.** `adb shell` склеивает аргументы в одну строку и отдаёт
//!    её шеллу устройства, поэтому квотируем сами. Без этого удаление
//!    `/sdcard/My Folder` удаляло `/sdcard/My` и `/sdcard/Folder` — это был
//!    реальный баг, а не теория.
//! 2. **Нормализация пути.** Проверки на системные каталоги делаются по
//!    схлопнутому пути, иначе `/sdcard/../system` обошёл бы их.
//! 3. **Разбор `ls -la`.** Имя может содержать пробелы, симлинк печатается как
//!    `name -> target`, а на Android `/sdcard` — симлинк, и без отдельной
//!    проверки он не открывался бы как каталог.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::devices::{self, AdbOutput};
use crate::error::ApiError;

/// Обычные операции над файлами — `timeout=30` в Python.
const SHELL_TIMEOUT: Duration = Duration::from_secs(30);

/// Проверка симлинков одной командой — `timeout=15`.
const LINK_TIMEOUT: Duration = Duration::from_secs(15);

/// `mv` — `timeout=60`: перенос между разделами копирует данные.
const MOVE_TIMEOUT: Duration = Duration::from_secs(60);

/// `push`/`pull` — `timeout=600`: гигабайтный файл по USB едет минутами.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(600);

/// `write` — `timeout=120`, как в Python: текстовый файл заведомо небольшой.
const WRITE_TIMEOUT: Duration = Duration::from_secs(120);

/// Потолок `max_bytes` у `read` — 1 МиБ.
const MAX_READ_BYTES: usize = 1024 * 1024;

/// Сколько читаем по умолчанию — `max_bytes=262144` в сигнатуре эндпоинта.
const DEFAULT_READ_BYTES: usize = 256 * 1024;

/// Размер страницы листинга по умолчанию и его потолок.
const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 500;

/// Точные пути, которые нельзя удалять, перемещать и перезаписывать.
const PROTECTED_ROOTS: [&str; 12] = [
    "/",
    "/system",
    "/data",
    "/vendor",
    "/sdcard",
    "/storage",
    "/storage/emulated",
    "/storage/emulated/0",
    "/storage/self",
    "/storage/self/primary",
    "/mnt",
    "/mnt/sdcard",
];

/// Целые поддеревья, куда запись и удаление запрещены.
const PROTECTED_PREFIXES: [&str; 7] =
    ["/system", "/vendor", "/proc", "/sys", "/dev", "/apex", "/boot"];

/// Запись каталога — поля `FileInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    permissions: String,
    date: String,
    is_link: bool,
    link_target: String,
}

impl FileInfo {
    fn to_value(&self) -> Value {
        json!({
            "name": self.name,
            "path": self.path,
            "is_dir": self.is_dir,
            "size": self.size,
            "permissions": self.permissions,
            "date": self.date,
            "is_link": self.is_link,
            "link_target": self.link_target,
        })
    }
}

// ---------------------------------------------------------------------------
// Пути и квотирование
// ---------------------------------------------------------------------------

/// Схлопывает `.`, `..` и повторные слэши — `_normalize_path`.
///
/// Проверки на системные каталоги идут по результату: иначе
/// `/sdcard/../system` прошёл бы мимо них.
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }

    let candidate = trimmed.replace('\\', "/");
    let candidate = if candidate.starts_with('/') {
        candidate
    } else {
        format!("/{candidate}")
    };

    // Аналог `posixpath.normpath` для абсолютного пути: пустые сегменты и `.`
    // выкидываем, `..` снимает предыдущий. Выше корня подняться нельзя.
    let mut stack: Vec<&str> = Vec::new();
    for segment in candidate.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }

    if stack.is_empty() {
        return "/".to_string();
    }
    format!("/{}", stack.join("/"))
}

/// Системный ли это путь — `_is_protected_path`.
pub fn is_protected_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    if PROTECTED_ROOTS.contains(&normalized.as_str()) {
        return true;
    }
    PROTECTED_PREFIXES
        .iter()
        .any(|prefix| normalized == *prefix || normalized.starts_with(&format!("{prefix}/")))
}

/// Базовое имя файла — `_safe_filename`.
///
/// `../../evil` не должен уйти за пределы каталога, поэтому от имени остаётся
/// только последний сегмент. Пустое, `.`, `..` и имя с нулевым байтом — 400.
pub fn safe_filename(name: &str) -> Result<String, ApiError> {
    let candidate = name.replace('\\', "/");
    let candidate = candidate.trim().rsplit('/').next().unwrap_or("").trim();

    if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || candidate.contains('\0')
    {
        return Err(ApiError::new(400, "Invalid file name"));
    }
    Ok(candidate.to_string())
}

/// Приклеивает имя к каталогу устройства — `_join_device_path`.
fn join_device_path(directory: &str, name: &str) -> String {
    format!("{}/{}", normalize_path(directory).trim_end_matches('/'), name)
}

/// Квотирование для шелла устройства — аналог `shlex.quote`.
///
/// Ровно та же таблица безопасных символов, что у Python: всё остальное
/// уезжает в одинарных кавычках, а сама кавычка разрывает строку по схеме
/// `'"'"'`.
pub fn sh_quote(value: &str) -> String {
    const SAFE: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_@%+=:,./-";

    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().all(|ch| SAFE.contains(ch)) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// Склеивает аргументы в строку для `adb shell` — `_shell_string`.
fn shell_string(args: &[&str]) -> String {
    args.iter()
        .map(|arg| sh_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `adb [-s serial] shell <строка>` — `_run_adb_shell`.
async fn run_shell(
    adb: &Path,
    serial: Option<&str>,
    args: &[&str],
    timeout: Duration,
) -> Result<AdbOutput, ApiError> {
    run_shell_raw(adb, serial, &shell_string(args), timeout).await
}

/// То же, но команда уже собрана — для скрипта проверки симлинков.
async fn run_shell_raw(
    adb: &Path,
    serial: Option<&str>,
    command: &str,
    timeout: Duration,
) -> Result<AdbOutput, ApiError> {
    let mut argv: Vec<&str> = Vec::new();
    if let Some(serial) = serial {
        argv.push("-s");
        argv.push(serial);
    }
    argv.push("shell");
    argv.push(command);
    devices::run_adb(adb, &argv, timeout).await
}

// ---------------------------------------------------------------------------
// Разбор `ls`
// ---------------------------------------------------------------------------

/// Разбирает вывод `ls -la` — `parse_ls_output`.
pub fn parse_ls_output(output: &str, base_path: &str) -> Vec<FileInfo> {
    let mut files = Vec::new();

    for line in output.trim().split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with("total ") || line.starts_with("ls:") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            continue;
        }

        let permissions = parts[0];
        // Имя — всё с восьмого поля: пробелы в именах не редкость.
        let mut name = if parts.len() > 7 {
            parts[7..].join(" ")
        } else {
            (*parts.last().expect("частей не меньше семи")).to_string()
        };

        let is_link = permissions.starts_with('l');
        let mut link_target = String::new();
        if is_link {
            if let Some((left, right)) = name.split_once(" -> ") {
                link_target = right.trim().to_string();
                name = left.trim().to_string();
            }
        }

        if name == "." || name == ".." {
            continue;
        }

        let size = parts[4].parse::<u64>().unwrap_or(0);
        let date = format!("{} {}", parts[5], parts[6]);

        files.push(FileInfo {
            path: format!("{}/{}", base_path.trim_end_matches('/'), name),
            name,
            is_dir: permissions.starts_with('d'),
            size,
            permissions: permissions.to_string(),
            date,
            is_link,
            link_target,
        });
    }

    files
}

/// Запасной разбор простого `ls -p` — `parse_simple_ls_output`.
pub fn parse_simple_ls_output(output: &str, base_path: &str) -> Vec<FileInfo> {
    output
        .trim()
        .split('\n')
        .filter_map(|line| {
            let name = line.trim();
            if name.is_empty() || name == "." || name == ".." || name.starts_with("ls:") {
                return None;
            }

            let is_dir = name.ends_with('/');
            let name = name.trim_end_matches('/').to_string();

            Some(FileInfo {
                path: format!("{}/{}", base_path.trim_end_matches('/'), name),
                name,
                is_dir,
                size: 0,
                permissions: String::new(),
                date: String::new(),
                is_link: false,
                link_target: String::new(),
            })
        })
        .collect()
}

/// Отмечает симлинки, ведущие в каталоги, — `_resolve_link_dirs`.
///
/// Одной командой на весь листинг, а не по вызову на ссылку: на `/` их
/// десяток, и каждый вызов adb — это десятки миллисекунд.
async fn resolve_link_dirs(adb: &Path, serial: Option<&str>, files: &mut [FileInfo]) {
    let script = files
        .iter()
        .filter(|item| item.is_link)
        .map(|item| {
            let quoted = sh_quote(&item.path);
            format!("if [ -d {quoted} ]; then echo {quoted}; fi")
        })
        .collect::<Vec<_>>()
        .join("; ");

    if script.is_empty() {
        return;
    }

    // Ошибку глотаем: непроверенные ссылки просто останутся файлами, ронять
    // из-за этого весь листинг незачем.
    let Ok(output) = run_shell_raw(adb, serial, &script, LINK_TIMEOUT).await else {
        return;
    };

    let dirs: Vec<&str> = output.stdout.lines().map(str::trim).collect();
    for item in files.iter_mut().filter(|item| item.is_link) {
        if dirs.contains(&item.path.as_str()) {
            item.is_dir = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Эндпоинты
// ---------------------------------------------------------------------------

/// Путь для `ls` — всегда с завершающей косой чертой.
///
/// Без неё `ls -la /sdcard` описывает **саму ссылку**, а не то, куда она ведёт:
/// на Android `/sdcard` — симлинк на `/storage/self/primary`, и файловый
/// менеджер открывался на списке из одной строки вместо содержимого папки.
/// Косая черта заставляет `ls` разыменовать ссылку.
///
/// Путь для дочерних записей при этом берётся исходный: иначе они получили бы
/// адреса вида `/sdcard//DCIM`.
fn list_target(path: &str) -> String {
    format!("{}/", path.trim_end_matches('/'))
}

/// Зеркалит `GET /api/files/list`.
///
/// Ошибки: 404 — пути нет, 403 — нет доступа, 400 — прочий отказ `ls`.
/// Разделение по тексту ошибки, а не по коду возврата: `ls` на всё отвечает
/// единицей.
pub async fn list(
    adb: &Path,
    path: &str,
    serial: Option<&str>,
    page: usize,
    page_size: usize,
) -> Result<Value, ApiError> {
    let target = list_target(path);
    let output = run_shell(adb, serial, &["ls", "-la", &target], SHELL_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Operation timed out"));
    }

    let stderr = output.stderr.trim();
    let mut ls_error = stderr.to_string();
    if ls_error.is_empty() {
        if let Some(line) = output.stdout.lines().find(|line| line.starts_with("ls:")) {
            ls_error = line.trim().to_string();
        }
    }
    if ls_error.is_empty() {
        if output.stdout.contains("Permission denied") {
            ls_error = "Permission denied".to_string();
        } else if output.stdout.contains("No such file") || output.stdout.contains("not a directory")
        {
            ls_error = "Path not found".to_string();
        }
    }

    if !output.success() || !ls_error.is_empty() {
        if ls_error.contains("No such file") || ls_error.contains("not a directory") {
            return Err(ApiError::new(404, ls_error));
        }
        if ls_error.contains("Permission denied") {
            return Err(ApiError::new(403, ls_error));
        }
        if !output.success() {
            return Err(ApiError::new(
                400,
                if ls_error.is_empty() {
                    "ls failed".to_string()
                } else {
                    ls_error
                },
            ));
        }
    }

    let mut files = parse_ls_output(&output.stdout, path);
    // Прошивки без `ls -la` (toybox в урезанной сборке) печатают простой
    // список — разбираем его вторым заходом.
    if files.is_empty() && !output.stdout.trim().is_empty() {
        let fallback = run_shell(adb, serial, &["ls", "-p", &target], SHELL_TIMEOUT).await?;
        files = parse_simple_ls_output(&fallback.stdout, path);
    }

    resolve_link_dirs(adb, serial, &mut files).await;

    // Каталоги первыми, дальше по имени без учёта регистра.
    files.sort_by(|left, right| {
        (!left.is_dir, left.name.to_lowercase()).cmp(&(!right.is_dir, right.name.to_lowercase()))
    });

    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
    let total_count = files.len();
    let total_pages = total_count.div_ceil(page_size).max(1);
    let page = page.max(1).min(total_pages);
    let start = (page - 1) * page_size;
    let page_items: Vec<Value> = files
        .iter()
        .skip(start)
        .take(page_size)
        .map(FileInfo::to_value)
        .collect();

    Ok(json!({
        "path": path,
        "count": page_items.len(),
        "files": page_items,
        "total_count": total_count,
        "page": page,
        "page_size": page_size,
        "total_pages": total_pages,
    }))
}

/// Зеркалит `DELETE /api/files/delete`.
pub async fn delete(adb: &Path, path: &str, serial: Option<&str>) -> Result<Value, ApiError> {
    if is_protected_path(path) {
        return Err(ApiError::new(400, "Cannot delete system directories"));
    }

    let output = run_shell(adb, serial, &["rm", "-rf", path], SHELL_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Delete operation timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, first_message(&output, "Delete failed")));
    }

    Ok(json!({"success": true, "path": path}))
}

/// Зеркалит `POST /api/files/mkdir`.
pub async fn mkdir(adb: &Path, path: &str, serial: Option<&str>) -> Result<Value, ApiError> {
    let output = run_shell(adb, serial, &["mkdir", "-p", path], SHELL_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Mkdir operation timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, first_message(&output, "Mkdir failed")));
    }

    Ok(json!({"success": true, "path": path}))
}

/// Зеркалит `POST /api/files/move` — перенос и переименование.
pub async fn move_path(
    adb: &Path,
    source: &str,
    destination: &str,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    if is_protected_path(source) || is_protected_path(destination) {
        return Err(ApiError::new(400, "Cannot move system directories"));
    }

    let output = run_shell(adb, serial, &["mv", source, destination], MOVE_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Move operation timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, output.stderr.clone()));
    }

    Ok(json!({"success": true, "path": destination}))
}

/// Зеркалит `GET /api/files/read` — начало файла с ограничением по размеру.
pub async fn read(
    adb: &Path,
    path: &str,
    max_bytes: usize,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    if max_bytes == 0 {
        return Err(ApiError::new(400, "max_bytes must be positive"));
    }
    let max_bytes = max_bytes.min(MAX_READ_BYTES);

    // `head -c` режет на устройстве; `cat` тянул бы файл целиком в память и
    // только потом отрезал первые 256 КБ.
    let limit = (max_bytes + 1).to_string();
    let mut output = run_shell(adb, serial, &["head", "-c", &limit, path], SHELL_TIMEOUT).await?;
    if !output.success() {
        // Урезанные прошивки бывают без `head -c`.
        let fallback = run_shell(adb, serial, &["cat", path], SHELL_TIMEOUT).await?;
        if !fallback.success() {
            return Err(ApiError::new(400, output.stderr.clone()));
        }
        output = fallback;
    }

    // Байты, а не символы: обрезка по символам разошлась бы с Python на
    // не-ASCII содержимом.
    //
    // Python режет байты до декодирования, здесь вывод уже декодирован lossy —
    // на двоичном файле счётчик получится больше (каждый битый байт стал
    // трёхбайтовым U+FFFD), и `truncated` может встать там, где Python его не
    // ставил. Для просмотрщика это не важно: двоичное содержимое он всё равно
    // не показывает, а на тексте расхождения нет.
    let raw = output.stdout.as_bytes();
    let truncated = raw.len() > max_bytes;
    let raw = if truncated { &raw[..max_bytes] } else { raw };

    Ok(json!({
        "path": path,
        "content": String::from_utf8_lossy(raw),
        "truncated": truncated,
        // Нулевой байт — признак того, что показывать это как текст бессмысленно.
        "is_binary": raw.contains(&0),
    }))
}

/// Зеркалит `POST /api/files/write` — текстовый файл на устройство.
pub async fn write(
    adb: &Path,
    path: &str,
    content: &str,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    if is_protected_path(path) {
        return Err(ApiError::new(400, "Cannot write to system directories"));
    }

    let name = safe_filename(&normalize_path(path))?;
    let temp = TempDir::new()?;
    let temp_path = temp.path().join(name);
    std::fs::write(&temp_path, content).map_err(|err| ApiError::internal(err.to_string()))?;

    let output = push(adb, &temp_path, path, serial, WRITE_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Write operation timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, output.stderr.clone()));
    }

    Ok(json!({
        "success": true,
        "path": path,
        "size": content.len(),
    }))
}

/// Зеркалит `POST /api/files/pull` — скачивание файла в локальный каталог.
pub async fn pull(
    adb: &Path,
    path: &str,
    target_dir: &Path,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    std::fs::create_dir_all(target_dir).map_err(|err| ApiError::new(400, err.to_string()))?;

    let target = target_dir.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = Vec::new();
    if let Some(serial) = serial {
        argv.push("-s");
        argv.push(serial);
    }
    argv.extend_from_slice(&["pull", path, &target]);

    let output = devices::run_adb(adb, &argv, TRANSFER_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Pull operation timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, first_message(&output, "Pull failed")));
    }

    // Имя берётся из исходного пути: adb кладёт файл в каталог под ним же.
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");

    Ok(json!({
        "success": true,
        "path": target_dir.join(name).to_string_lossy(),
    }))
}

/// Зеркалит `POST /api/files/upload`, но принимает **путь на диске**, а не
/// содержимое файла.
///
/// Это единственное место порта, где контракт REST не повторён дословно.
/// Причина в транспорте: HTTP-версия принимает `multipart/form-data` из
/// `<input type=file>`, а через IPC содержимое файла не передать — фронтенд в
/// десктопе берёт путь через `@tauri-apps/plugin-dialog` и отдаёт его сюда.
/// Веб-панель продолжает слать multipart в Python.
pub async fn upload(
    adb: &Path,
    source: &Path,
    destination: &str,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    let metadata = std::fs::metadata(source)
        .map_err(|err| ApiError::new(400, format!("{}: {err}", source.display())))?;
    if !metadata.is_file() {
        return Err(ApiError::new(400, "Only files can be uploaded"));
    }

    let name = safe_filename(&source.file_name().unwrap_or_default().to_string_lossy())?;
    let dest_dir = normalize_path(destination);
    if is_protected_path(&dest_dir) {
        return Err(ApiError::new(400, "Cannot upload to system directories"));
    }
    let dest_path = join_device_path(&dest_dir, &name);

    let output = push(adb, source, &dest_path, serial, TRANSFER_TIMEOUT).await?;
    if output.timed_out {
        return Err(ApiError::internal("Upload timed out"));
    }
    if !output.success() {
        return Err(ApiError::new(400, output.stderr.clone()));
    }

    Ok(json!({
        "success": true,
        "path": dest_path,
        "filename": name,
        "size": metadata.len(),
    }))
}

/// `adb push <source> <destination>`.
async fn push(
    adb: &Path,
    source: &Path,
    destination: &str,
    serial: Option<&str>,
    timeout: Duration,
) -> Result<AdbOutput, ApiError> {
    let source = source.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = Vec::new();
    if let Some(serial) = serial {
        argv.push("-s");
        argv.push(serial);
    }
    argv.extend_from_slice(&["push", &source, destination]);

    devices::run_adb(adb, &argv, timeout).await
}

/// Первое непустое из stderr/stdout, иначе запасной текст.
fn first_message(output: &AdbOutput, fallback: &str) -> String {
    let stderr = output.stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    let stdout = output.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    fallback.to_string()
}

/// Временный каталог, который убирается за собой.
///
/// `tempfile` ради одного вызова не тащим: имя строится из pid и счётчика,
/// а `create_dir` падает, если такой каталог уже есть, — то есть коллизия не
/// молчаливая.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, ApiError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let path = std::env::temp_dir().join(format!(
            "mkdsc-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).map_err(|err| ApiError::internal(err.to_string()))?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Без уборки копия оставалась бы в %TEMP% навсегда — этот баг в Python
        // чинили background-задачей у `FileResponse`.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Разбирает `page`/`page_size` из строки запроса.
///
/// Нечисловое значение — значение по умолчанию, как у FastAPI, который на
/// мусоре в query отвечает 422; ронять листинг из-за этого незачем.
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

/// `max_bytes` из строки запроса.
pub fn read_limit(query: &std::collections::HashMap<String, String>) -> usize {
    query
        .get("max_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_READ_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_dots_and_slashes() {
        assert_eq!(normalize_path("/sdcard//DCIM/"), "/sdcard/DCIM");
        assert_eq!(normalize_path("/sdcard/../system"), "/system");
        assert_eq!(normalize_path("sdcard/DCIM"), "/sdcard/DCIM");
        assert_eq!(normalize_path("\\sdcard\\DCIM"), "/sdcard/DCIM");
        assert_eq!(normalize_path("/sdcard/./x"), "/sdcard/x");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("   "), "/");
        assert_eq!(normalize_path("/"), "/");
        // Выше корня не поднимаемся.
        assert_eq!(normalize_path("/../.."), "/");
        // Две ведущие косые: `posixpath.normpath` их бы сохранил, Python снимает
        // лишнюю отдельным циклом — итог тот же.
        assert_eq!(normalize_path("//sdcard"), "/sdcard");
    }

    #[test]
    fn protected_paths_cover_roots_and_subtrees() {
        for path in [
            "/",
            "/system",
            "/data",
            "/sdcard",
            "/storage/emulated/0",
            "/mnt/sdcard",
            "/proc/1",
            "/sys/class",
            "/dev/null",
            "/apex/whatever",
            "/boot",
            // Обход схлопыванием не проходит.
            "/sdcard/../system",
            "/sdcard/",
        ] {
            assert!(is_protected_path(path), "должен быть защищён: {path}");
        }

        for path in [
            "/sdcard/DCIM",
            "/data/local/tmp/file",
            "/storage/emulated/0/Download",
            "/systemctl",
        ] {
            assert!(!is_protected_path(path), "не должен быть защищён: {path}");
        }
    }

    /// Квотирование — то самое место, где ломалось удаление файлов с пробелом.
    #[test]
    fn quoting_matches_shlex() {
        assert_eq!(sh_quote("simple"), "simple");
        assert_eq!(sh_quote("/sdcard/DCIM"), "/sdcard/DCIM");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("/sdcard/My Folder"), "'/sdcard/My Folder'");
        assert_eq!(sh_quote("a;rm -rf /"), "'a;rm -rf /'");
        assert_eq!(sh_quote("it's"), r#"'it'"'"'s'"#);
        assert_eq!(sh_quote("$(id)"), "'$(id)'");

        assert_eq!(
            shell_string(&["rm", "-rf", "/sdcard/My Folder"]),
            "rm -rf '/sdcard/My Folder'"
        );
    }

    #[test]
    fn safe_filename_keeps_only_the_last_segment() {
        assert_eq!(safe_filename("photo.png").unwrap(), "photo.png");
        assert_eq!(safe_filename("../../evil.sh").unwrap(), "evil.sh");
        assert_eq!(safe_filename("C:\\tmp\\a.txt").unwrap(), "a.txt");
        for bad in ["", "   ", ".", "..", "/", "dir/"] {
            assert_eq!(
                safe_filename(bad).unwrap_err().detail,
                "Invalid file name",
                "должно отвергаться: {bad:?}"
            );
        }
    }

    #[test]
    fn parses_ls_la_output() {
        let output = "total 32\n\
drwxrwx--x 2 root sdcard_rw 4096 2026-08-13 20:11 DCIM\n\
-rw-rw---- 1 root sdcard_rw  1234 2026-08-13 20:12 note.txt\n\
-rw-rw---- 1 root sdcard_rw  4096 2026-08-13 20:13 My Report.pdf\n\
lrwxrwxrwx 1 root root         21 2026-08-13 20:14 sdcard -> /storage/emulated/0\n\
.\n";

        let files = parse_ls_output(output, "/sdcard");
        assert_eq!(files.len(), 4);

        assert_eq!(files[0].name, "DCIM");
        assert!(files[0].is_dir);
        assert_eq!(files[0].path, "/sdcard/DCIM");
        assert_eq!(files[0].date, "2026-08-13 20:11");

        assert_eq!(files[1].size, 1234);
        assert!(!files[1].is_dir);

        // Пробел в имени не должен обрезать имя.
        assert_eq!(files[2].name, "My Report.pdf");
        assert_eq!(files[2].path, "/sdcard/My Report.pdf");

        // Симлинк: цель отдельно от имени.
        assert!(files[3].is_link);
        assert_eq!(files[3].name, "sdcard");
        assert_eq!(files[3].link_target, "/storage/emulated/0");
    }

    #[test]
    fn ls_parser_skips_noise() {
        assert!(parse_ls_output("", "/sdcard").is_empty());
        assert!(parse_ls_output("total 0\n", "/sdcard").is_empty());
        assert!(parse_ls_output("ls: /nope: No such file or directory", "/nope").is_empty());
        // Короткие строки без даты и размера разбору не поддаются.
        assert!(parse_ls_output("drwx 2 root root\n", "/sdcard").is_empty());
        // `.` и `..` не попадают в листинг.
        let dots = "drwxrwx--x 2 root root 4096 2026-08-13 20:11 .\n\
drwxrwx--x 2 root root 4096 2026-08-13 20:11 ..\n";
        assert!(parse_ls_output(dots, "/sdcard").is_empty());
    }

    #[test]
    fn parses_simple_ls_output() {
        let files = parse_simple_ls_output("DCIM/\nnote.txt\n\nls: bad\n.\n", "/sdcard");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "DCIM");
        assert!(files[0].is_dir);
        assert_eq!(files[1].name, "note.txt");
        assert!(!files[1].is_dir);
        assert_eq!(files[1].path, "/sdcard/note.txt");

        // Причуда оригинала: `.` отсеивается до снятия косой черты, поэтому
        // `./` проходит фильтр и остаётся записью с именем `.`. Повторяем как
        // есть — `ls -p` текущий каталог не печатает, и на практике сюда не
        // попадает, а расходиться с Python на ровном месте незачем.
        let dot = parse_simple_ls_output("./\n", "/sdcard");
        assert_eq!(dot.len(), 1);
        assert_eq!(dot[0].name, ".");
    }

    /// Косая черта в конце — то, что заставляет `ls` открыть симлинк
    /// `/sdcard`, а не описать его самого.
    #[test]
    fn listing_always_dereferences_the_directory() {
        assert_eq!(list_target("/sdcard"), "/sdcard/");
        assert_eq!(list_target("/sdcard/"), "/sdcard/");
        assert_eq!(list_target("/sdcard/DCIM"), "/sdcard/DCIM/");
        // Корень не должен превратиться в `//`.
        assert_eq!(list_target("/"), "/");
    }

    #[test]
    fn device_paths_are_joined_without_double_slashes() {
        assert_eq!(join_device_path("/sdcard", "a.txt"), "/sdcard/a.txt");
        assert_eq!(join_device_path("/sdcard/", "a.txt"), "/sdcard/a.txt");
        assert_eq!(join_device_path("/", "a.txt"), "/a.txt");
    }

    #[test]
    fn paging_falls_back_to_defaults() {
        let mut query = std::collections::HashMap::new();
        assert_eq!(paging(&query), (1, DEFAULT_PAGE_SIZE));

        query.insert("page".to_string(), "3".to_string());
        query.insert("page_size".to_string(), "10".to_string());
        assert_eq!(paging(&query), (3, 10));

        query.insert("page".to_string(), "0".to_string());
        query.insert("page_size".to_string(), "9000".to_string());
        assert_eq!(paging(&query), (1, MAX_PAGE_SIZE));

        query.insert("page".to_string(), "abc".to_string());
        query.insert("page_size".to_string(), "".to_string());
        assert_eq!(paging(&query), (1, DEFAULT_PAGE_SIZE));
    }

    #[test]
    fn temp_dir_cleans_up_after_itself() {
        let path = {
            let temp = TempDir::new().expect("создан");
            let path = temp.path().to_path_buf();
            std::fs::write(path.join("x.txt"), "x").expect("записан");
            assert!(path.is_dir());
            path
        };
        assert!(!path.exists(), "временный каталог остался: {}", path.display());
    }
}
