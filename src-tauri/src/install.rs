//! Установка adb и scrcpy — порт скачивающей половины `mkdsc/tools.py`.
//!
//! Здесь три вещи, которые нельзя упрощать:
//!
//! 1. **Версии закреплены.** `platform-tools-latest` меняется под нами и не
//!    имеет опубликованной суммы, а мажорный релиз scrcpy переименовывает
//!    флаги, которыми приложение управляет продуктом. Обе ссылки указывают на
//!    конкретную ревизию.
//! 2. **Сумма проверяется до распаковки.** Иначе подмену архива (MITM,
//!    компрометация зеркала, кривой редирект) заметить нечем.
//! 3. **Права на POSIX.** Zip не обязан нести режимы файлов, и распакованный
//!    без бита исполнения adb превращал первый запуск в бесконечную
//!    перекачку platform-tools.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tauri::AppHandle;

use crate::paths;
use crate::tools;

/// Таймаут запроса к API GitHub — `timeout=10` в Python.
const API_TIMEOUT: Duration = Duration::from_secs(10);

/// Таймаут установления соединения на скачивание — `timeout=30`.
///
/// Именно на соединение, а не на всю загрузку: архив в полторы сотни мегабайт
/// на медленном канале едет дольше любого разумного общего таймаута.
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Проверка запуска инструмента (`adb version`) — как `DEFAULT_CMD_TIMEOUT`.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(20);

const ANDROID_REPO: &str = "https://dl.google.com/android/repository";

/// Ревизия platform-tools и её суммы.
///
/// Google публикует в манифесте только SHA-1, поэтому SHA-256 зафиксированы
/// здесь; порядок обновления описан в `docs/build.md`.
const PLATFORM_TOOLS_REVISION: &str = "37.0.1";

const PLATFORM_TOOLS_WINDOWS_SHA256: &str =
    "45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99";
const PLATFORM_TOOLS_LINUX_SHA256: &str =
    "d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1";

/// Закреплённая версия scrcpy.
///
/// Раньше здесь был `releases/latest`, и каждая новая установка получала
/// версию, на которой приложение никто не проверял.
const SCRCPY_PINNED_VERSION: &str = "4.1";

const SCRCPY_RELEASES_API: &str = "https://api.github.com/repos/Genymobile/scrcpy/releases";
const SCRCPY_CHECKSUMS_ASSET: &str = "SHA256SUMS.txt";

/// Диапазон версий scrcpy, на котором проверен набор флагов приложения.
const SCRCPY_VERIFIED_MIN: (u32, u32, u32) = (3, 3, 4);
const SCRCPY_VERIFIED_MAX: (u32, u32) = (4, 1);

/// Прогресс подготовки инструментов — то, что отдаёт `GET /api/bootstrap/status`.
#[derive(Debug, Clone)]
pub struct Status {
    /// `idle`, `checking`, `downloading`, `verifying`, `extracting`, `ready`.
    pub stage: &'static str,
    pub tool: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub scrcpy_version: String,
    /// Непустая строка, если версия scrcpy вне проверенного диапазона.
    pub scrcpy_version_warning: String,
    /// Текст сбоя подготовки. Интерфейс показывает его вместо прогресса.
    pub error: Option<String>,
    pub ready: bool,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            stage: "idle",
            tool: String::new(),
            downloaded_bytes: 0,
            total_bytes: 0,
            scrcpy_version: String::new(),
            scrcpy_version_warning: String::new(),
            error: None,
            ready: false,
        }
    }
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);

/// Текущее состояние подготовки.
pub fn status() -> Status {
    STATUS
        .lock()
        .ok()
        .and_then(|status| status.clone())
        .unwrap_or_default()
}

fn update(change: impl FnOnce(&mut Status)) {
    if let Ok(mut guard) = STATUS.lock() {
        let status = guard.get_or_insert_with(Status::default);
        change(status);
    }
}

/// Готовит adb и scrcpy: находит на диске, а если их нет — скачивает.
///
/// Вызывается один раз при старте приложения, в фоне: интерфейс к этому моменту
/// уже открыт и показывает прогресс.
pub async fn ensure_tools(app: &AppHandle) {
    update(|status| {
        status.stage = "checking";
        status.tool = "adb".to_string();
        status.error = None;
    });

    if let Err(error) = ensure_adb(app).await {
        return fail(error);
    }

    update(|status| {
        status.stage = "checking";
        status.tool = "scrcpy".to_string();
    });

    let scrcpy = match ensure_scrcpy(app).await {
        Ok(path) => path,
        Err(error) => return fail(error),
    };

    // Версия читается уже у готового бинаря — в том числе у чужого, на который
    // указали через `MKDSC_SCRCPY_PATH`: как раз ради этого случая проверка и
    // существует.
    let (version, warning) = scrcpy_version_of(&scrcpy).await;
    update(|status| {
        status.stage = "ready";
        status.tool = String::new();
        status.downloaded_bytes = 0;
        status.total_bytes = 0;
        status.scrcpy_version = version;
        status.scrcpy_version_warning = warning;
        status.ready = true;
    });
}

fn fail(error: String) {
    update(|status| {
        status.stage = "error";
        status.error = Some(error);
    });
}

async fn ensure_adb(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = tools::adb_path(app) {
        ensure_executable(&path);
        if verify(&path, "version").await {
            return Ok(path);
        }
    }

    let (url, sha256) = platform_tools_source()?;
    let archive = format!("platform-tools-{PLATFORM_TOOLS_REVISION}.zip");
    download_and_extract(app, "platform-tools", &url, &archive, sha256).await?;

    let path = tools::adb_path(app).ok_or("adb not found after download")?;
    ensure_executable(&path);
    Ok(path)
}

async fn ensure_scrcpy(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = tools::scrcpy_path(app) {
        ensure_executable(&path);
        if verify(&path, "--version").await {
            return Ok(path);
        }
    }

    let (name, url, sha256) = scrcpy_source().await?;
    download_and_extract(app, "scrcpy", &url, &name, &sha256).await?;

    let path = tools::scrcpy_path(app).ok_or("scrcpy not found after download")?;
    ensure_executable(&path);
    Ok(path)
}

/// Ссылка и сумма архива platform-tools для текущей платформы.
///
/// macOS не поддерживается — сборок под неё нет, и притворяться, что есть,
/// незачем.
fn platform_tools_source() -> Result<(String, &'static str), String> {
    if cfg!(windows) {
        Ok((
            format!("{ANDROID_REPO}/platform-tools_r{PLATFORM_TOOLS_REVISION}-win.zip"),
            PLATFORM_TOOLS_WINDOWS_SHA256,
        ))
    } else if cfg!(target_os = "linux") {
        Ok((
            format!("{ANDROID_REPO}/platform-tools_r{PLATFORM_TOOLS_REVISION}-linux.zip"),
            PLATFORM_TOOLS_LINUX_SHA256,
        ))
    } else {
        Err("Unsupported platform for adb".to_string())
    }
}

/// Имя, ссылка и сумма архива scrcpy из закреплённого релиза.
///
/// Без опубликованной суммы установка прекращается: молча ставить
/// непроверенный бинарь — ровно то, что здесь чинилось.
async fn scrcpy_source() -> Result<(String, String, String), String> {
    let version = scrcpy_target_version();
    let release: Value = get_json(&format!("{SCRCPY_RELEASES_API}/tags/v{version}")).await?;

    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or("scrcpy release has no assets")?;

    let checksums_url = assets
        .iter()
        .find(|asset| asset_name(asset) == SCRCPY_CHECKSUMS_ASSET)
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let names: Vec<String> = assets
        .iter()
        .map(asset_name)
        .filter(|name| name != SCRCPY_CHECKSUMS_ASSET)
        .collect();

    let name = select_scrcpy_asset(&names)
        .ok_or("scrcpy archive not found for this platform")?
        .to_string();

    let url = assets
        .iter()
        .find(|asset| asset_name(asset) == name)
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(Value::as_str)
        .ok_or("scrcpy asset has no download url")?
        .to_string();

    let checksums_url = checksums_url.ok_or_else(|| missing_checksum_message(&name))?;
    let checksums = get_text(&checksums_url).await?;
    let sha256 = parse_sha256sums(&checksums, &name).ok_or_else(|| missing_checksum_message(&name))?;

    Ok((name, url, sha256))
}

fn missing_checksum_message(name: &str) -> String {
    format!(
        "No SHA-256 checksum published for {name}. Set MKDSC_SCRCPY_VERSION to a release that \
         ships {SCRCPY_CHECKSUMS_ASSET}, or point MKDSC_SCRCPY_PATH at your own scrcpy."
    )
}

fn asset_name(asset: &Value) -> String {
    asset
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Версия scrcpy, которую надо поставить.
///
/// `MKDSC_SCRCPY_VERSION` позволяет взять свою, не пересобирая приложение, —
/// например, откатиться, если в закреплённой нашёлся баг.
fn scrcpy_target_version() -> String {
    std::env::var("MKDSC_SCRCPY_VERSION")
        .ok()
        .map(|value| value.trim().trim_start_matches(['v', 'V']).to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| SCRCPY_PINNED_VERSION.to_string())
}

/// Выбирает архив под текущую платформу — `_select_scrcpy_asset`.
fn select_scrcpy_asset(names: &[String]) -> Option<&str> {
    let (prefix, arch) = if cfg!(windows) {
        ("scrcpy-win", "win64")
    } else {
        ("scrcpy-linux", "x86_64")
    };

    let candidates: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| {
            let lowered = name.to_lowercase();
            lowered.starts_with(prefix)
                // Серверный jar — не наш архив.
                && !lowered.contains("server")
                && (lowered.ends_with(".zip")
                    || lowered.ends_with(".tar.gz")
                    || lowered.ends_with(".tgz"))
        })
        .collect();

    candidates
        .iter()
        .find(|name| name.to_lowercase().contains(arch))
        .or_else(|| candidates.first())
        .copied()
}

/// Достаёт сумму нужного файла из `SHA256SUMS.txt`.
///
/// Формат `sha256sum`: `<hex>  <имя>`; в бинарном режиме имя идёт со звёздочкой.
fn parse_sha256sums(text: &str, wanted: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let digest = parts.next()?.trim().to_lowercase();
        let name = parts.next()?.trim_start_matches('*');
        if parts.next().is_some() || name != wanted {
            return None;
        }
        (digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit())).then_some(digest)
    })
}

/// Качает архив, сверяет сумму и распаковывает в каталог загрузок.
async fn download_and_extract(
    app: &AppHandle,
    tool: &str,
    url: &str,
    archive_name: &str,
    expected_sha256: &str,
) -> Result<(), String> {
    let downloads = paths::downloads_dir(app);
    std::fs::create_dir_all(&downloads).map_err(|err| err.to_string())?;
    let archive = downloads.join(archive_name);

    download(url, &archive, tool).await?;

    update(|status| {
        status.stage = "verifying";
        status.tool = tool.to_string();
    });
    if let Err(error) = verify_sha256(&archive, expected_sha256) {
        // Битый файл убираем, чтобы следующий запуск не пытался распаковать
        // его повторно.
        let _ = std::fs::remove_file(&archive);
        return Err(error);
    }

    update(|status| {
        status.stage = "extracting";
        status.tool = tool.to_string();
    });
    let result = extract(&archive, &downloads);
    let _ = std::fs::remove_file(&archive);
    result
}

/// Скачивает файл, обновляя прогресс.
async fn download(url: &str, dest: &Path, tool: &str) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let client = reqwest::Client::builder()
        .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
        .user_agent(concat!("mkdsc-tauri/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| err.to_string())?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;

    let total = response.content_length().unwrap_or(0);
    update(|status| {
        status.stage = "downloading";
        status.tool = tool.to_string();
        status.downloaded_bytes = 0;
        status.total_bytes = total;
    });

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|err| err.to_string())?;
    let mut stream = response.bytes_stream();
    let mut received = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| err.to_string())?;
        file.write_all(&chunk).await.map_err(|err| err.to_string())?;
        received += chunk.len() as u64;
        update(|status| status.downloaded_bytes = received);
    }

    file.flush().await.map_err(|err| err.to_string())?;
    Ok(())
}

/// Сверяет контрольную сумму архива до распаковки.
fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    // Кусками: архивы весят десятки мегабайт, целиком в память их класть незачем.
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let actual = hex(&hasher.finalize());
    if actual != expected.trim().to_lowercase() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        return Err(format!(
            "Checksum mismatch for {name}: expected {expected}, got {actual}. \
             Download was discarded."
        ));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn extract(archive: &Path, dest: &Path) -> Result<(), String> {
    let name = archive.to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        return extract_zip(archive, dest);
    }
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return extract_tar_gz(archive, dest);
    }
    Err(format!(
        "Unsupported archive format: {}",
        archive.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// Распаковывает zip, не выпуская записи за пределы каталога.
fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|err| err.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|err| err.to_string())?;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|err| err.to_string())?;
        // `enclosed_name` — и есть защита от Zip Slip: запись с `..` или
        // абсолютным путём даёт None.
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe archive entry: {}", entry.name()))?;
        let target = dest.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|err| err.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        let mut out = std::fs::File::create(&target).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|err| err.to_string())?;

        // Режимы zip не обязан нести, но если несёт — применяем: без бита
        // исполнения adb на Linux не запускался, и приложение перекачивало
        // platform-tools по кругу.
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(mode));
        }
    }

    Ok(())
}

/// Распаковывает tar.gz — в этом виде scrcpy выходит на Linux.
fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|err| err.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    tar.set_preserve_permissions(true);

    for entry in tar.entries().map_err(|err| err.to_string())? {
        let mut entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path().map_err(|err| err.to_string())?.into_owned();
        // `unpack_in` сам отвергает `..` и абсолютные пути, возвращая `false`.
        if !entry.unpack_in(dest).map_err(|err| err.to_string())? {
            return Err(format!("Unsafe archive entry: {}", path.display()));
        }
    }

    Ok(())
}

/// Доставляет бит исполнения на POSIX, если его нет.
///
/// Страховка к режимам из архива: их может не оказаться (архив собран на
/// Windows, пересобран зеркалом), и тогда бинарь окажется 0644 и просто не
/// запустится.
fn ensure_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let Ok(metadata) = std::fs::metadata(path) else {
            return;
        };
        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o111));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Запускается ли инструмент — `_verify_tool`.
async fn verify(path: &Path, arg: &str) -> bool {
    crate::devices::run_adb(path, &[arg], VERIFY_TIMEOUT)
        .await
        .is_ok_and(|output| output.success())
}

/// Версия scrcpy и предупреждение о ней.
async fn scrcpy_version_of(path: &Path) -> (String, String) {
    let Ok(output) = crate::devices::run_adb(path, &["--version"], VERIFY_TIMEOUT).await else {
        return (String::new(), String::new());
    };

    let version = parse_scrcpy_version(&format!("{}\n{}", output.stdout, output.stderr));
    let text = version.map(format_version).unwrap_or_default();
    (text, scrcpy_version_warning(version))
}

/// Версия из вывода `scrcpy --version`.
///
/// Первая строка выглядит так: `scrcpy 4.1 <https://github.com/...>`.
fn parse_scrcpy_version(output: &str) -> Option<(u32, u32, u32)> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| line.to_lowercase().starts_with("scrcpy"))?;

    // Первая группа цифр, разделённых точками, — версия. Регулярка ради этого
    // не нужна: достаточно пройти по символам.
    let digits: String = line
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();

    let mut parts = digits.split('.').filter_map(|part| part.parse::<u32>().ok());
    let major = parts.next()?;
    let minor = parts.next()?;
    Some((major, minor, parts.next().unwrap_or(0)))
}

fn format_version(version: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// Текст предупреждения, если версия вне проверенного диапазона.
///
/// Пустая строка — всё в порядке. Предупреждение намеренно не блокирует запуск:
/// свою версию могли поставить сознательно.
fn scrcpy_version_warning(version: Option<(u32, u32, u32)>) -> String {
    let Some(version) = version else {
        return String::new();
    };
    if version >= SCRCPY_VERIFIED_MIN && (version.0, version.1) <= SCRCPY_VERIFIED_MAX {
        return String::new();
    }

    let min = format_version(SCRCPY_VERIFIED_MIN);
    format!(
        "scrcpy {} is outside the tested range {min}-{SCRCPY_PINNED_VERSION}; \
         some options may not work",
        format_version(version)
    )
}

async fn get_json(url: &str) -> Result<Value, String> {
    serde_json::from_str(&get_text(url).await?).map_err(|err| err.to_string())
}

async fn get_text(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(API_TIMEOUT)
        // GitHub отвечает 403 на запрос без User-Agent.
        .user_agent(concat!("mkdsc-tauri/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| err.to_string())?;

    client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .text()
        .await
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sha256sums() {
        let text = "\
abc  short.zip
2b8ec4e0b8c9c9f3a9a5b0c1d2e3f40516273849506172839405162738495061  scrcpy-linux-x86_64-v4.1.tar.gz
0000000000000000000000000000000000000000000000000000000000000000 *scrcpy-win64-v4.1.zip
not-a-digest  scrcpy-broken.zip
";
        assert_eq!(
            parse_sha256sums(text, "scrcpy-linux-x86_64-v4.1.tar.gz").as_deref(),
            Some("2b8ec4e0b8c9c9f3a9a5b0c1d2e3f40516273849506172839405162738495061")
        );
        // Звёздочка перед именем — бинарный режим sha256sum.
        assert_eq!(
            parse_sha256sums(text, "scrcpy-win64-v4.1.zip").as_deref(),
            Some("0000000000000000000000000000000000000000000000000000000000000000")
        );
        // Короткая или нешестнадцатеричная сумма не годится.
        assert_eq!(parse_sha256sums(text, "short.zip"), None);
        assert_eq!(parse_sha256sums(text, "scrcpy-broken.zip"), None);
        assert_eq!(parse_sha256sums(text, "nothing.zip"), None);
        assert_eq!(parse_sha256sums("", "any.zip"), None);
    }

    #[test]
    fn selects_the_archive_for_this_platform() {
        let names: Vec<String> = [
            "scrcpy-server-v4.1",
            "scrcpy-linux-x86_64-v4.1.tar.gz",
            "scrcpy-linux-aarch64-v4.1.tar.gz",
            "scrcpy-win64-v4.1.zip",
            "scrcpy-win32-v4.1.zip",
            "scrcpy-macos-aarch64-v4.1.tar.gz",
        ]
        .iter()
        .map(|name| (*name).to_string())
        .collect();

        let selected = select_scrcpy_asset(&names).expect("архив найден");
        if cfg!(windows) {
            assert_eq!(selected, "scrcpy-win64-v4.1.zip");
        } else {
            assert_eq!(selected, "scrcpy-linux-x86_64-v4.1.tar.gz");
        }

        // Серверный jar не архив с бинарём, а пустой список — не паника.
        assert_eq!(select_scrcpy_asset(&["scrcpy-server-v4.1".to_string()]), None);
        assert_eq!(select_scrcpy_asset(&[]), None);
    }

    #[test]
    fn parses_scrcpy_version() {
        assert_eq!(
            parse_scrcpy_version("scrcpy 4.1 <https://github.com/Genymobile/scrcpy>"),
            Some((4, 1, 0))
        );
        assert_eq!(parse_scrcpy_version("scrcpy 3.3.4"), Some((3, 3, 4)));
        assert_eq!(
            parse_scrcpy_version("noise\n  scrcpy 2.0.1 \nmore"),
            Some((2, 0, 1))
        );
        assert_eq!(parse_scrcpy_version("no version here"), None);
        assert_eq!(parse_scrcpy_version(""), None);
    }

    #[test]
    fn warns_only_outside_the_verified_range() {
        assert_eq!(scrcpy_version_warning(Some((4, 1, 0))), "");
        assert_eq!(scrcpy_version_warning(Some((3, 3, 4))), "");
        assert_eq!(scrcpy_version_warning(Some((4, 1, 7))), "");
        assert_eq!(scrcpy_version_warning(None), "");

        assert!(scrcpy_version_warning(Some((3, 3, 3))).contains("outside the tested range"));
        assert!(scrcpy_version_warning(Some((5, 0, 0))).contains("scrcpy 5.0.0"));
    }

    #[test]
    fn checksum_mismatch_is_reported_with_both_sums() {
        let path = std::env::temp_dir().join(format!("mkdsc-sha-{}.bin", std::process::id()));
        std::fs::write(&path, b"hello").expect("файл");

        // SHA-256 от "hello".
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&path, expected).is_ok());
        assert!(verify_sha256(&path, &expected.to_uppercase()).is_ok());

        let error = verify_sha256(&path, "0".repeat(64).as_str()).expect_err("не совпало");
        assert!(error.contains("Checksum mismatch"), "текст: {error}");
        assert!(error.contains(expected), "в тексте нет настоящей суммы: {error}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn version_override_comes_from_the_environment() {
        assert_eq!(scrcpy_target_version(), SCRCPY_PINNED_VERSION);
    }
}
