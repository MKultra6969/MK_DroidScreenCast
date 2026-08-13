//! Готовность инструментов — `GET /api/bootstrap/status`.
//!
//! Единственная команда порта, которая ходит обратно в Python. Причина в том,
//! что скачивание adb и scrcpy остаётся там до вехи 7, а прогресс живёт в
//! памяти скачивающего процесса: сколько байт из скольких получено, какой
//! инструмент качается, какая версия scrcpy и не выходит ли она за
//! проверенный диапазон. Ответить на это, глядя только на диск, нельзя, а без
//! ответа первый запуск показывал бы «подготовка» вместо процента все те
//! несколько минут, что идёт загрузка ~150 МБ.
//!
//! Поэтому здесь прокси: спрашиваем бэкенд, а если он ещё не поднялся или
//! молчит — отвечаем тем, что видно с нашей стороны (оба бинаря на диске —
//! значит, готово). Веха 7 забирает загрузку в Rust, и прокси уходит вместе с
//! Python.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::AppHandle;

use crate::error::ApiError;
use crate::tools;

/// Таймаут запроса к бэкенду.
///
/// Меньше, чем `timeoutMs: 4000` у фронтенда: интерфейс опрашивает статус в
/// цикле, и лучше отдать запасной ответ, чем задержать следующий опрос.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// Зеркалит `GET /api/bootstrap/status` — форма ответа та же.
pub async fn status(app: &AppHandle) -> Result<Value, ApiError> {
    if let Some(status) = ask_backend().await {
        return Ok(status);
    }

    let ready = tools::adb_path(app).is_some() && tools::scrcpy_path(app).is_some();
    Ok(local_status(ready))
}

/// Спрашивает бэкенд. `None` — не ответил или ответил не объектом.
async fn ask_backend() -> Option<Value> {
    let response = client()
        .get(format!(
            "http://127.0.0.1:{}/api/bootstrap/status",
            crate::BACKEND_PORT
        ))
        // Бэкенд требует токен на каждом запросе — тот же, что он получил при
        // запуске через окружение.
        .header(crate::TOKEN_HEADER, crate::api_token())
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;

    match response.json::<Value>().await {
        Ok(status @ Value::Object(_)) => Some(status),
        _ => None,
    }
}

/// Ответ по тому, что видно с нашей стороны: оба бинаря на диске — готово.
///
/// Прогресса здесь нет и быть не может, поэтому стадия — «стартуем»: пока
/// бэкенд не отвечает, ничего более точного мы не знаем.
fn local_status(ready: bool) -> Value {
    json!({
        "ready": ready,
        "error": Value::Null,
        "stage": if ready { "ready" } else { "starting" },
        "tool": "",
        "downloaded_bytes": 0,
        "total_bytes": 0,
        "scrcpy_version": "",
        "scrcpy_version_warning": "",
    })
}

/// Общий клиент: статус опрашивается в цикле, и собирать пул соединений на
/// каждый опрос незачем.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            // Сборка клиента по умолчанию не падает; если всё же упала —
            // клиент без настроек лучше, чем паника в команде.
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Запасной ответ обязан иметь все поля: интерфейс читает из него `stage`,
    /// `tool`, `downloaded_bytes` и `total_bytes` без проверок на наличие.
    #[test]
    fn local_status_has_every_field_the_ui_reads() {
        let status = local_status(false);
        assert_eq!(status["ready"], json!(false));
        assert_eq!(status["stage"], json!("starting"));

        let ready = local_status(true);
        assert_eq!(ready["ready"], json!(true));
        assert_eq!(ready["stage"], json!("ready"));
        // Без ошибки: `data.error` фронтенд показывает как текст сбоя запуска.
        assert_eq!(ready["error"], Value::Null);

        for key in [
            "ready",
            "error",
            "stage",
            "tool",
            "downloaded_bytes",
            "total_bytes",
            "scrcpy_version",
            "scrcpy_version_warning",
        ] {
            assert!(status.get(key).is_some(), "поле {key} потерялось");
        }
    }
}
