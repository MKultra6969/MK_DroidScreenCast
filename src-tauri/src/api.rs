//! IPC-команды `api_*` — зеркало HTTP-эндпоинтов бэкенда.
//!
//! Имя команды выводится из метода и пути (`GET /api/devices` →
//! `api_devices`), форма ответа и коды ошибок повторяют FastAPI один в один.
//! Благодаря этому фронтенд переезжает на IPC, не меняя ни одного из 40+ мест
//! вызова `apiFetch`: вся развилка живёт в `frontend/src/lib/api.ts`.
//!
//! Команды объявлены `async`, хотя работа с конфигом блокирующая: так Tauri
//! уводит их с главного потока. Файл размером с килобайт того стоит, а
//! `spawn_blocking` на каждый вызов только добавил бы шума.

use serde_json::{Map, Value, json};
use tauri::AppHandle;

use crate::error::ApiError;
use crate::{config, devices, tools};

/// Языки веб-панели — ключи `LEXICON_WEB` из `mkdsc/i18n/lexicon_web.py`.
///
/// Сам словарь (508 строк) остаётся в Python вместе с `GET /api/i18n`; сюда
/// нужен только список. Чтобы он не разъехался со словарём, есть тест
/// `languages_match_python_lexicon`.
const LANGUAGES: [&str; 2] = ["en", "ru"];

/// Зеркалит `GET /api/devices` (`mkdsc/web/server.py`).
///
/// Возвращает `{"saved": [...], "connected": [...], "timestamp": "..."}`:
/// `saved` — как есть из `config.json`, `connected` — разбор `adb devices`,
/// `timestamp` — ISO-8601 без таймзоны.
///
/// Если adb ещё не найден (Python его докачивает), `connected` — пустой
/// список, а не ошибка: ровно так ведёт себя и HTTP-эндпоинт, пока
/// `app.state.adb_path` не выставлен.
///
/// Ошибка запуска adb — `ApiError` со статусом 500.
#[tauri::command]
pub async fn api_devices(app: AppHandle) -> Result<Value, ApiError> {
    let saved = config::saved_devices(&app)?;

    let connected = match tools::adb_path(&app) {
        Some(adb) => devices::list_connected(&adb).await?,
        None => Vec::new(),
    };

    Ok(json!({
        "saved": saved,
        "connected": connected,
        "timestamp": now_iso(),
    }))
}

/// Зеркалит `GET /api/config`.
///
/// Отдаёт не весь конфиг, а срез, который нужен интерфейсу, плюс версию
/// приложения и список языков.
#[tauri::command]
pub async fn api_config(app: AppHandle) -> Result<Value, ApiError> {
    let config = config::load(&app)?;
    Ok(config_summary(&config))
}

/// Зеркалит `GET /api/config/full` — конфиг целиком, для встроенного
/// редактора.
#[tauri::command]
pub async fn api_config_full(app: AppHandle) -> Result<Value, ApiError> {
    Ok(Value::Object(config::load(&app)?))
}

/// Зеркалит `GET /api/presets` — список пресетов scrcpy.
#[tauri::command]
pub async fn api_presets(app: AppHandle) -> Result<Value, ApiError> {
    let config = config::load(&app)?;
    Ok(config::presets_of(&config))
}

// ---------------------------------------------------------------------------
// Вспомогательное
// ---------------------------------------------------------------------------

/// Срез конфига для `GET /api/config` — форма из `mkdsc/web/server.py`.
fn config_summary(config: &Map<String, Value>) -> Value {
    json!({
        "language": config.get("language").cloned().unwrap_or_else(|| json!("en")),
        "presets": config::presets_of(config),
        "web": section(config, "web"),
        "logs": section(config, "logs"),
        "downloads": section(config, "downloads"),
        "connection_optimizer": section(config, "connection_optimizer"),
        "recording": section(config, "recording"),
        // Версия берётся из `Cargo.toml`; тест `version_matches_python_constant`
        // в `config.rs` следит, чтобы она не разъехалась с `mkdsc/constants.py`.
        "version": env!("CARGO_PKG_VERSION"),
        "languages": LANGUAGES,
    })
}

fn section(config: &Map<String, Value>, name: &str) -> Value {
    config.get(name).cloned().unwrap_or_else(|| json!({}))
}

/// Локальное время без таймзоны с микросекундами — формат
/// `datetime.now().isoformat()`, который отдаёт тот же эндпоинт в Python.
fn now_iso() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Список языков обязан совпадать с ключами `LEXICON_WEB`: он уезжает в
    /// `GET /api/config` и определяет содержимое переключателя языка, а сам
    /// словарь пока живёт в Python и меняется независимо.
    #[test]
    fn languages_match_python_lexicon() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri лежит в корне репозитория")
            .join("mkdsc")
            .join("i18n")
            .join("lexicon_web.py");
        let source = std::fs::read_to_string(&path).expect("lexicon_web.py на месте");

        // Ключи верхнего уровня — единственные строки с ровно одним отступом,
        // после которых открывается словарь. Вложенные ключи отбиты глубже, а
        // их значения — строки, не `{`.
        let languages: Vec<String> = source
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("    \"")?;
                let (language, tail) = rest.split_once('"')?;
                let tail = tail.trim_start().strip_prefix(':')?;
                tail.trim_start()
                    .starts_with('{')
                    .then(|| language.to_string())
            })
            .collect();

        assert!(!languages.is_empty(), "разбор lexicon_web.py ничего не нашёл");
        assert_eq!(languages, LANGUAGES.to_vec());
    }

    #[test]
    fn config_summary_mirrors_rest_shape() {
        let config = config::default_config();
        let summary = config_summary(&config);

        assert_eq!(summary["language"], json!("en"));
        assert_eq!(summary["presets"], config::default_presets());
        assert_eq!(summary["web"]["port"], json!(6969));
        assert_eq!(summary["languages"], json!(["en", "ru"]));
        assert_eq!(summary["version"], json!(env!("CARGO_PKG_VERSION")));
        // Секции уезжают целиком: интерфейс читает из них отдельные поля.
        for key in [
            "logs",
            "downloads",
            "connection_optimizer",
            "recording",
            "web",
        ] {
            assert!(summary[key].is_object(), "секция {key} потерялась");
        }
        // Конфиг целиком сюда не попадает — за этим есть `/api/config/full`.
        assert!(summary.get("devices").is_none());
        assert!(summary.get("scrcpy").is_none());
    }

}
