//! Проверка обновлений — порт `mkdsc/updater.py` и `mkdsc/versioning.py`.
//!
//! Речь только о **проверке**: устанавливает обновление `tauri-plugin-updater`
//! из подписанного фида, и фронтенд ходит к нему сам. Отсюда он получает
//! описание релиза и ссылку на страницу — на них он откатывается, когда фид
//! про новую версию ещё не знает или подписи нет вовсе.

use std::time::Duration;

use serde_json::{Value, json};

use crate::error::ApiError;

/// Куда отправлять пользователя, если у релиза нет своей страницы.
///
/// Адреса записаны целиком, а не собраны из владельца и имени репозитория:
/// склеить их в `const` без внешнего крейта нельзя, а тест
/// `repo_constants_match_python` всё равно сверяет их с `mkdsc/constants.py`.
const RELEASES_URL: &str = "https://github.com/MKultra6969/MK_DroidScreenCast/releases/latest";

/// API последнего релиза — `API_LATEST_RELEASE` в `mkdsc/constants.py`.
const API_LATEST_RELEASE: &str =
    "https://api.github.com/repos/MKultra6969/MK_DroidScreenCast/releases/latest";

/// Таймаут запроса — `requests.get(..., timeout=10)`.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Зеркалит `GET /api/update/check`.
///
/// Возвращает `{current, latest, update_available, release, release_url}`.
/// Сетевая ошибка — 500 с текстом от клиента: так же ведёт себя и Python,
/// где исключение `requests` доезжает до обработчика ошибок.
pub async fn check() -> Result<Value, ApiError> {
    let release = fetch_latest_release().await?;
    let latest = release.get("tag").and_then(Value::as_str);
    let update_available = latest.is_some_and(|latest| is_newer(env!("CARGO_PKG_VERSION"), latest));
    let release_url = release
        .get("html_url")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .unwrap_or(RELEASES_URL)
        .to_string();

    Ok(json!({
        "current": env!("CARGO_PKG_VERSION"),
        "latest": latest,
        "update_available": update_available,
        "release": release,
        "release_url": release_url,
    }))
}

/// Забирает последний релиз с GitHub и раскладывает его в форму
/// `fetch_latest_release`.
async fn fetch_latest_release() -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        // GitHub отвечает 403 на запрос без User-Agent. `requests` подставлял
        // свой, а `reqwest` не подставляет ничего.
        .user_agent(concat!("mkdsc-tauri/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| ApiError::internal(err.to_string()))?;

    let response = client
        .get(API_LATEST_RELEASE)
        .send()
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?
        .error_for_status()
        .map_err(|err| ApiError::internal(err.to_string()))?;

    let payload: Value = response
        .json()
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?;

    Ok(release_of(&payload))
}

/// Срез ответа GitHub, который уезжает фронтенду.
fn release_of(payload: &Value) -> Value {
    let field = |name: &str| payload.get(name).cloned().unwrap_or(Value::Null);

    let assets: Vec<Value> = payload
        .get("assets")
        .and_then(Value::as_array)
        .map(|assets| {
            assets
                .iter()
                .map(|asset| {
                    json!({
                        "name": asset.get("name").cloned().unwrap_or(Value::Null),
                        "url": asset.get("browser_download_url").cloned().unwrap_or(Value::Null),
                        "size": asset.get("size").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    json!({
        "tag": field("tag_name"),
        "name": field("name"),
        // Описание релиза показывается в диалоге обновления, и `null` там
        // выглядел бы как строка «null» — Python тоже приводит его к пустой.
        "body": payload.get("body").and_then(Value::as_str).unwrap_or_default(),
        "published_at": field("published_at"),
        "zipball_url": field("zipball_url"),
        "html_url": field("html_url"),
        "assets": assets,
        "fetched_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false),
    })
}

/// Новее ли `latest`, чем `current`, — `is_newer` из `mkdsc/versioning.py`.
fn is_newer(current: &str, latest: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

/// Версия как список чисел: `v1.0.2` → `[1, 0, 2]`.
///
/// Всё нечисловое работает разделителем, поэтому `1.0.2-beta3` превращается в
/// `[1, 0, 2, 3]`. Сравнение списков лексикографическое — как у кортежей в
/// Python, откуда это и взято.
fn parse_version(version: &str) -> Vec<u64> {
    version
        .trim()
        .to_lowercase()
        .trim_start_matches('v')
        .split(|ch: char| !ch.is_ascii_digit())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_matches_python() {
        assert!(is_newer("1.0.2", "1.0.3"));
        assert!(is_newer("1.0.2", "v1.1.0"));
        assert!(is_newer("1.0.2", "2.0"));
        assert!(!is_newer("1.0.2", "1.0.2"));
        assert!(!is_newer("1.0.2", "1.0.1"));
        assert!(!is_newer("1.0.2", ""));
        // Короткий кортеж меньше своего продолжения — как в Python.
        assert!(is_newer("1.0", "1.0.1"));
        assert!(!is_newer("1.0.1", "1.0"));
    }

    #[test]
    fn version_parsing_matches_python() {
        assert_eq!(parse_version("v1.0.2"), vec![1, 0, 2]);
        assert_eq!(parse_version(" 1.0.2 "), vec![1, 0, 2]);
        assert_eq!(parse_version("V1.0.2-beta3"), vec![1, 0, 2, 3]);
        assert_eq!(parse_version(""), Vec::<u64>::new());
        assert_eq!(parse_version("release"), Vec::<u64>::new());
    }

    #[test]
    fn release_shape_mirrors_python() {
        let payload = json!({
            "tag_name": "v1.0.3",
            "name": "1.0.3",
            "body": "notes",
            "published_at": "2026-01-01T00:00:00Z",
            "zipball_url": "https://example.invalid/zip",
            "html_url": "https://example.invalid/release",
            "assets": [{
                "name": "app.AppImage",
                "browser_download_url": "https://example.invalid/app",
                "size": 42,
            }],
        });

        let release = release_of(&payload);
        assert_eq!(release["tag"], json!("v1.0.3"));
        assert_eq!(release["body"], json!("notes"));
        assert_eq!(release["assets"][0]["url"], json!("https://example.invalid/app"));
        assert_eq!(release["assets"][0]["size"], json!(42));
        assert!(release["fetched_at"].as_str().is_some_and(|at| at.ends_with("+00:00")));

        // Пустой ответ не должен ронять разбор: полей может не быть вообще.
        let empty = release_of(&json!({}));
        assert_eq!(empty["tag"], Value::Null);
        assert_eq!(empty["body"], json!(""));
        assert_eq!(empty["assets"], json!([]));
    }

    /// Адреса репозитория обязаны совпадать с `mkdsc/constants.py`: релизы
    /// публикуются в одно место, а проверок теперь две.
    #[test]
    fn repo_constants_match_python() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri лежит в корне репозитория")
            .join("mkdsc")
            .join("constants.py");
        let source = std::fs::read_to_string(&path).expect("constants.py на месте");

        let value_of = |name: &str| {
            source
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{name} = \"")))
                .and_then(|rest| rest.split('"').next())
                .map(str::to_string)
                .unwrap_or_else(|| panic!("{name} не найден"))
        };

        let owner = value_of("REPO_OWNER");
        let repo = value_of("REPO_NAME");

        assert_eq!(
            RELEASES_URL,
            format!("https://github.com/{owner}/{repo}/releases/latest")
        );
        assert_eq!(
            API_LATEST_RELEASE,
            format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
        );
    }
}
