//! Готовность инструментов — `GET /api/bootstrap/status`.
//!
//! Раньше эта команда ходила по HTTP в Python: скачивал инструменты он, и
//! прогресс жил в его памяти. Теперь скачивает `install.rs`, и статус берётся
//! прямо оттуда.

use serde_json::{Value, json};

use crate::error::ApiError;
use crate::install;

/// Зеркалит `GET /api/bootstrap/status` — форма ответа не менялась с тех пор,
/// как её отдавал Python: интерфейс разбирает `ready`, `error`, `stage`,
/// `tool`, `downloaded_bytes`, `total_bytes` и предупреждение о версии scrcpy.
pub fn status() -> Result<Value, ApiError> {
    let status = install::status();

    Ok(json!({
        "ready": status.ready,
        "error": status.error,
        "stage": status.stage,
        "tool": status.tool,
        "downloaded_bytes": status.downloaded_bytes,
        "total_bytes": status.total_bytes,
        // Версия scrcpy вне проверенного диапазона не блокирует запуск, но
        // пользователь должен знать, почему часть опций может не работать.
        "scrcpy_version": status.scrcpy_version,
        "scrcpy_version_warning": status.scrcpy_version_warning,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Интерфейс читает эти поля без проверок на наличие: недостающее поле
    /// превратится в `undefined` и уедет в разметку как «undefined».
    #[test]
    fn status_has_every_field_the_ui_reads() {
        let payload = status().expect("статус");

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
            assert!(payload.get(key).is_some(), "поле {key} потерялось");
        }

        // До запуска подготовки — «не готово» и без ошибки: интерфейс покажет
        // экран загрузки, а не сбой.
        assert_eq!(payload["ready"], json!(false));
        assert_eq!(payload["error"], Value::Null);
    }
}
