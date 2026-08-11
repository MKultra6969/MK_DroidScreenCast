//! Чтение `config.json`.
//!
//! Только чтение. Пока Python-бэкенд жив, он остаётся единственным писателем:
//! запись у него атомарная (`os.replace`), но это защищает от обрыва записи,
//! а не от гонки двух процессов — второй писатель просто затирал бы правки
//! первого. Запись, валидация и миграция переезжают в Rust отдельной вехой,
//! вместе с удалением Python.

use serde_json::Value;
use tauri::AppHandle;

use crate::paths;

/// Читает `config.json` целиком.
///
/// Отсутствующий или битый файл — не ошибка, а `Value::Null`: бэкенд стартует
/// первым и создаёт конфиг сам, а битый он переименовывает в бэкап и
/// пересоздаёт. Падать здесь означало бы показывать ошибку в UI ровно в тот
/// момент, когда Python уже всё чинит.
pub fn read_config(app: &AppHandle) -> Value {
    let path = paths::config_path(app);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

/// Сохранённые устройства из конфига — как есть, без нормализации.
///
/// Форму записей задаёт `mkdsc/devices.py::save_device`; фронтенд терпим к
/// лишним полям, но `port` обязан остаться строкой.
pub fn saved_devices(app: &AppHandle) -> Vec<Value> {
    let config = read_config(app);
    config
        .get("devices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
