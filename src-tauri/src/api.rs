//! IPC-команды `api_*` — зеркало HTTP-эндпоинтов бэкенда.
//!
//! Имя команды выводится из метода и пути (`GET /api/devices` →
//! `api_devices`), форма ответа и коды ошибок повторяют FastAPI один в один.
//! Благодаря этому фронтенд переезжает на IPC, не меняя ни одного из 40+ мест
//! вызова `apiFetch`: вся развилка живёт в `frontend/src/lib/api.ts`.

use serde_json::{Value, json};
use tauri::AppHandle;

use crate::error::ApiError;
use crate::{config, devices, tools};

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
    let saved = config::saved_devices(&app);

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

/// Локальное время без таймзоны с микросекундами — формат
/// `datetime.now().isoformat()`, который отдаёт тот же эндпоинт в Python.
fn now_iso() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string()
}
