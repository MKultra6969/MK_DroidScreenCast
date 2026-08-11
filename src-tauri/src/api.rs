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

use std::collections::HashMap;

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

/// Зеркалит `POST /api/config`: частичный патч.
///
/// Присланные ключи сливаются с текущим конфигом. Удалить ключ патчем нельзя —
/// для этого есть `PUT`. Тело не объект — 400 `Config payload required`.
#[tauri::command]
pub async fn api_config_update(app: AppHandle, body: Option<Value>) -> Result<Value, ApiError> {
    let patch = require_object(body)?;
    let config = config::update(&app, &patch)?;
    Ok(json!({"success": true, "config": config}))
}

/// Зеркалит `PUT /api/config`: полная замена.
///
/// Ключи, стёртые в редакторе, действительно исчезают. Патч этого не умел:
/// пользователь удалял ключ, видел «Config saved», а ключ возвращался.
#[tauri::command]
pub async fn api_config_replace(app: AppHandle, body: Option<Value>) -> Result<Value, ApiError> {
    let new_config = require_object(body)?;
    let config = config::replace(&app, &new_config)?;
    Ok(json!({"success": true, "config": config}))
}

/// Зеркалит `GET /api/presets` — список пресетов scrcpy.
#[tauri::command]
pub async fn api_presets(app: AppHandle) -> Result<Value, ApiError> {
    let config = config::load(&app)?;
    Ok(config::presets_of(&config))
}

/// Зеркалит `POST /api/presets`: добавляет пресет или обновляет одноимённый.
///
/// Сравнение имён регистронезависимое — как в Python. Пустое имя — 400
/// `Name required`.
#[tauri::command]
pub async fn api_presets_save(app: AppHandle, body: Option<Value>) -> Result<Value, ApiError> {
    let data = object_or_empty(body);

    let Some(name) = data
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
    else {
        return Err(ApiError::new(400, "Name required"));
    };

    let bitrate = data.get("bitrate").cloned().unwrap_or(Value::Null);
    let maxsize = data.get("maxsize").cloned().unwrap_or(Value::Null);
    let lowered = name.to_lowercase();

    let config = config::mutate(&app, move |config| {
        let presets = presets_mut(config);
        let existing = presets
            .iter()
            .position(|preset| preset_name(preset).map(str::to_lowercase).as_deref() == Some(&lowered));

        match existing {
            // `preset.update({...})` в Python: существующие ключи меняются на
            // месте, отсутствующие дописываются в конец записи.
            Some(index) => {
                if let Some(preset) = presets[index].as_object_mut() {
                    preset.insert("bitrate".to_string(), bitrate);
                    preset.insert("maxsize".to_string(), maxsize);
                }
            }
            None => presets.push(json!({
                "name": name,
                "bitrate": bitrate,
                "maxsize": maxsize,
            })),
        }

        Ok(())
    })?;

    Ok(json!({"success": true, "presets": config::presets_of(&config)}))
}

/// Зеркалит `DELETE /api/presets/{name}`.
///
/// Имя приходит из пути уже раскодированным (`decodeURIComponent` в роутере).
/// В ответе — оставшиеся пресеты.
#[tauri::command]
pub async fn api_presets_delete(
    app: AppHandle,
    params: HashMap<String, String>,
) -> Result<Value, ApiError> {
    let lowered = param(&params, "name").to_lowercase();

    let config = config::mutate(&app, move |config| {
        let presets = presets_mut(config);
        presets
            .retain(|preset| preset_name(preset).map(str::to_lowercase).as_deref() != Some(&lowered));
        Ok(())
    })?;

    Ok(json!({"success": true, "presets": config::presets_of(&config)}))
}

/// Зеркалит `POST /api/devices/save`.
///
/// Устройство опознаётся парой «адрес + порт»: совпало — обновляем имя, тип
/// подключения и время последнего использования, нет — дописываем запись.
/// Без имени или адреса — 400 `Name and IP required`.
#[tauri::command]
pub async fn api_devices_save(app: AppHandle, body: Option<Value>) -> Result<Value, ApiError> {
    let data = object_or_empty(body);

    let name = data.get("name").cloned().unwrap_or(Value::Null);
    let ip = data.get("ip").cloned().unwrap_or(Value::Null);
    if !config::is_truthy(&name) || !config::is_truthy(&ip) {
        return Err(ApiError::new(400, "Name and IP required"));
    }

    // Порт хранится строкой: фронтенд сравнивает его со строкой из серийника
    // вида `192.168.1.5:5555`, и число вместо строки тихо ломало бы сравнение.
    let port = config::python_str(&data.get("port").cloned().unwrap_or_else(|| json!("5555")));
    let connection_type = data.get("type").cloned().unwrap_or_else(|| json!("wifi"));
    let message = format!("Device '{}' saved", config::python_str(&name));

    config::mutate(&app, move |config| {
        let devices = devices_mut(config);
        let existing = devices
            .iter()
            .position(|device| device.get("ip") == Some(&ip) && device_port(device) == port);

        match existing {
            Some(index) => {
                if let Some(device) = devices[index].as_object_mut() {
                    device.insert("name".to_string(), name);
                    device.insert("connection_type".to_string(), connection_type);
                    device.insert("last_used".to_string(), json!(now_iso()));
                }
            }
            None => devices.push(json!({
                "name": name,
                "ip": ip,
                "port": port,
                "connection_type": connection_type,
                "last_used": now_iso(),
            })),
        }

        Ok(())
    })?;

    Ok(json!({"success": true, "message": message}))
}

/// Зеркалит `DELETE /api/devices/{ip}/{port}`.
#[tauri::command]
pub async fn api_devices_delete(
    app: AppHandle,
    params: HashMap<String, String>,
) -> Result<Value, ApiError> {
    let ip = param(&params, "ip").to_string();
    let port = param(&params, "port").to_string();

    config::mutate(&app, move |config| {
        let devices = devices_mut(config);
        devices.retain(|device| {
            !(device.get("ip").and_then(Value::as_str) == Some(ip.as_str())
                && device_port(device) == port)
        });
        Ok(())
    })?;

    Ok(json!({"success": true}))
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

/// Параметр пути. Роутер всегда их передаёт, но отсутствие не должно ронять
/// команду — пустое значение просто ни с чем не совпадёт.
fn param<'a>(params: &'a HashMap<String, String>, name: &str) -> &'a str {
    params.get(name).map(String::as_str).unwrap_or_default()
}

/// Тело запроса как объект; иначе 400 с тем же текстом, что у FastAPI.
fn require_object(body: Option<Value>) -> Result<Map<String, Value>, ApiError> {
    match body {
        Some(Value::Object(map)) => Ok(map),
        _ => Err(ApiError::new(400, "Config payload required")),
    }
}

/// Тело запроса как объект; не объект — пустой.
///
/// Так срабатывают проверки обязательных полей и пользователь видит
/// осмысленный текст ошибки, а не «поле body некорректно».
fn object_or_empty(body: Option<Value>) -> Map<String, Value> {
    match body {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// Имя пресета, если оно строка.
///
/// В Python здесь `preset.get("name", "").lower()`, и на нестроковом имени
/// эндпоинт упал бы с 500. Валидация конфига такие записи не пропускает, так
/// что нестроковое имя тут просто ни с чем не совпадает.
fn preset_name(preset: &Value) -> Option<&str> {
    preset.as_object()?.get("name")?.as_str()
}

/// Порт устройства в том же виде, в каком его пишет `api_devices_save`.
fn device_port(device: &Value) -> String {
    config::python_str(device.get("port").unwrap_or(&Value::Null))
}

/// Изменяемый список пресетов. Секции, испорченные вручную, восстанавливаются:
/// команда не должна падать на конфиге, который до неё уже починила миграция.
fn presets_mut(config: &mut Map<String, Value>) -> &mut Vec<Value> {
    let scrcpy = config.entry("scrcpy").or_insert_with(|| json!({}));
    if !scrcpy.is_object() {
        *scrcpy = json!({});
    }
    let scrcpy = scrcpy.as_object_mut().expect("секция приведена к объекту");

    let presets = scrcpy.entry("presets").or_insert_with(|| json!([]));
    if !presets.is_array() {
        *presets = json!([]);
    }
    presets.as_array_mut().expect("пресеты приведены к списку")
}

/// Изменяемый список сохранённых устройств.
fn devices_mut(config: &mut Map<String, Value>) -> &mut Vec<Value> {
    let devices = config.entry("devices").or_insert_with(|| json!([]));
    if !devices.is_array() {
        *devices = json!([]);
    }
    devices.as_array_mut().expect("устройства приведены к списку")
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

    #[test]
    fn require_object_rejects_non_objects() {
        assert!(require_object(Some(json!({"a": 1}))).is_ok());
        for body in [None, Some(Value::Null), Some(json!([1, 2])), Some(json!("x"))] {
            let error = require_object(body).expect_err("не объект");
            assert_eq!(error.status, 400);
            assert_eq!(error.detail, "Config payload required");
        }
    }

    #[test]
    fn presets_and_devices_are_recovered_from_broken_sections() {
        let mut config = config::default_config();
        config.insert("scrcpy".to_string(), Value::Null);
        config.insert("devices".to_string(), json!("not a list"));

        presets_mut(&mut config).push(json!({"name": "Mine"}));
        devices_mut(&mut config).push(json!({"ip": "10.0.0.2", "port": "5555"}));

        assert_eq!(config["scrcpy"]["presets"], json!([{"name": "Mine"}]));
        assert_eq!(config["devices"][0]["ip"], json!("10.0.0.2"));
    }

    #[test]
    fn device_port_is_always_compared_as_string() {
        assert_eq!(device_port(&json!({"port": "5555"})), "5555");
        assert_eq!(device_port(&json!({"port": 5555})), "5555");
        assert_eq!(device_port(&json!({})), "None");
    }
}
