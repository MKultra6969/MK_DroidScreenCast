//! Владение `config.json`: чтение, миграция и атомарная запись.
//!
//! Порт `mkdsc/config.py`. С этой вехи файл принадлежит Rust: Python-бэкенд
//! запускается с `MKDSC_CONFIG_READONLY=1` и только читает. Двух писателей быть
//! не должно — атомарная запись спасает от обрыва посреди файла, но не от того,
//! что второй процесс перезапишет чужие правки целиком.
//!
//! Работа идёт через `serde_json::Value`, а не через типизированные структуры,
//! и с включённой фичей `preserve_order`. Причина: конфиг правится руками и
//! через встроенный редактор, а порядок ключей в файле должен оставаться тем
//! же, что писал Python (`json.dumps` по словарю с порядком вставки). С
//! дефолтным `BTreeMap` первая же запись отсортировала бы файл по алфавиту:
//! пользовательские правки перемешались бы, а каждый следующий diff стал бы
//! нечитаемым.
//!
//! Отдельно: `Map::remove` при `preserve_order` работает как `swap_remove` и
//! рвёт порядок. Здесь он не используется намеренно.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::{Map, Value, json};
use tauri::AppHandle;

use crate::error::ApiError;
use crate::paths;

/// Версия схемы конфига. Зеркалит `mkdsc/constants.py::CONFIG_SCHEMA_VERSION`.
pub const CONFIG_SCHEMA_VERSION: i64 = 1;

/// Секции, которые обязаны быть объектами; всё прочее сбрасывается в дефолт.
const DICT_SECTIONS: [&str; 7] = [
    "web",
    "logs",
    "downloads",
    "connection_optimizer",
    "recording",
    "cli",
    "scrcpy",
];

/// Процессный замок вокруг пары «чтение — изменение — запись».
///
/// Без него `POST /api/presets` и `POST /api/devices/save`, пришедшие
/// одновременно, прочитали бы один и тот же конфиг и второй затёр бы правку
/// первого. В Python ту же роль играет `_CONFIG_LOCK`.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// Счётчик для имён временных файлов — уникальность в пределах процесса.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Пути, от которых зависит работа с конфигом.
///
/// Отдельная структура, а не `AppHandle`, чтобы логика была проверяема тестами:
/// `AppHandle` в юнит-тесте не создать.
pub struct ConfigPaths {
    /// Сам `config.json`.
    pub config: PathBuf,
    /// `devices.json` — формат до появления `config.json`; читается один раз
    /// при миграции.
    pub legacy_devices: PathBuf,
}

/// Пути конфига для запущенного приложения.
pub fn config_paths(app: &AppHandle) -> ConfigPaths {
    ConfigPaths {
        config: paths::config_path(app),
        legacy_devices: paths::legacy_devices_path(app),
    }
}

fn lock() -> MutexGuard<'static, ()> {
    // Отравленный замок восстанавливаем: под ним лежит `()`, инвариантов,
    // которые мог бы нарушить паникующий поток, просто нет.
    CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Значения по умолчанию
// ---------------------------------------------------------------------------

/// Пресеты scrcpy по умолчанию — `mkdsc/config.py::DEFAULT_PRESETS`.
pub fn default_presets() -> Value {
    json!([
        {"name": "FullHD", "bitrate": "8M", "maxsize": "1080"},
        {"name": "2K", "bitrate": "16M", "maxsize": "1440"},
        {"name": "4K", "bitrate": "32M", "maxsize": "2160"},
    ])
}

/// Конфиг по умолчанию — `mkdsc/config.py::DEFAULT_CONFIG`.
///
/// Порядок ключей здесь задаёт порядок ключей в файле, поэтому он повторяет
/// питоновский буквально.
pub fn default_config() -> Map<String, Value> {
    let value = json!({
        "config_version": CONFIG_SCHEMA_VERSION,
        "language": "en",
        "web": {
            "host": "127.0.0.1",
            "port": 6969,
            "auto_open": true,
        },
        "logs": {
            "export_dir": "",
        },
        "downloads": {
            "base_dir": "",
        },
        "connection_optimizer": {
            "auto_switch": false,
        },
        "recording": {
            "output_dir": "",
            "format": "mp4",
            "audio_source": "output",
            "file_prefix": "recording",
            "show_preview": true,
            "stay_awake": false,
            "show_touches": false,
            "turn_screen_off": false,
        },
        "cli": {
            "show_banner": true,
        },
        "scrcpy": {
            "bitrate": "8M",
            "maxsize": "1080",
            "keyboard": "uhid",
            "presets": default_presets(),
            "stay_awake": false,
            "show_touches": false,
            "fullscreen": false,
            "no_audio": false,
            "turn_screen_off": false,
        },
        "devices": [],
        "last_update_check": null,
    });

    match value {
        Value::Object(map) => map,
        _ => unreachable!("литерал выше — объект"),
    }
}

// ---------------------------------------------------------------------------
// Питоновская семантика на значениях JSON
// ---------------------------------------------------------------------------

/// Истинность значения по правилам Python: `None`, `false`, `0`, `""`, `[]`,
/// `{}` — ложь.
///
/// Нужна дословно: миграция построена на проверках вида `if not
/// config["scrcpy"].get("presets")`, и «пустой список» там значит то же, что
/// «ключа нет».
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_none_or(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Аналог `int(value)`: `bool` — 0/1, число — усечение к нулю, строка —
/// разбор. Всё остальное — `None`, как `TypeError`/`ValueError` в Python.
fn python_int(value: &Value) -> Option<i64> {
    match value {
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::Number(number) => match number.as_i64() {
            Some(exact) => Some(exact),
            // `int(6969.9) == 6969`; строку с точкой Python бы не принял, и
            // `parse::<i64>` тоже не принимает — расхождения нет.
            None => number.as_f64().map(|n| n.trunc() as i64),
        },
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Аналог `str(value)` для тех типов, что доезжают в JSON-теле запроса.
///
/// Нужен ради `save_device`, где порт приводится к строке: фронтенд шлёт
/// строку, но сторонний клиент может прислать число, и в конфиге оно обязано
/// оказаться строкой — иначе сравнение с сохранённым портом перестанет
/// срабатывать.
pub fn python_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        other => other.to_string(),
    }
}

/// Слияние словарей — `mkdsc/config.py::_deep_merge`.
///
/// Вложенные объекты сливаются рекурсивно, всё остальное (в том числе списки)
/// заменяется целиком. Удалить ключ патчем нельзя — ради удаления существует
/// отдельный `PUT /api/config`, который заменяет конфиг полностью.
///
/// Порядок ключей: сначала порядок `defaults`, затем то, чего в нём не было.
fn deep_merge(defaults: &Map<String, Value>, overrides: &Map<String, Value>) -> Map<String, Value> {
    let mut result = Map::new();

    for (key, value) in defaults {
        match overrides.get(key) {
            Some(override_value) => {
                let merged = match (value, override_value) {
                    (Value::Object(base), Value::Object(patch)) => {
                        Value::Object(deep_merge(base, patch))
                    }
                    _ => override_value.clone(),
                };
                result.insert(key.clone(), merged);
            }
            None => {
                result.insert(key.clone(), value.clone());
            }
        }
    }

    for (key, value) in overrides {
        if !result.contains_key(key) {
            result.insert(key.clone(), value.clone());
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Валидация и миграция
// ---------------------------------------------------------------------------

/// Секция, которую `sanitize_config` уже привёл к объекту.
fn section_mut<'a>(config: &'a mut Map<String, Value>, name: &str) -> &'a mut Map<String, Value> {
    config
        .get_mut(name)
        .and_then(Value::as_object_mut)
        .expect("секцию привёл к объекту sanitize_config")
}

/// Приводит каждую известную секцию к ожидаемому типу —
/// `mkdsc/config.py::_sanitize_config`.
///
/// Сохранённый руками (или через редактор в UI) `"scrcpy": null` раньше ронял
/// каждый последующий `.get()`, и приложение оставалось нерабочим до ручной
/// правки файла. Поэтому проверок именно столько: каждая закрывает свой способ
/// это повторить. Возвращает конфиг и признак «пришлось чинить».
fn sanitize_config(config: Value) -> (Map<String, Value>, bool) {
    let defaults = default_config();

    let Value::Object(mut config) = config else {
        return (defaults, true);
    };

    let mut changed = false;

    for section in DICT_SECTIONS {
        if !config.get(section).is_some_and(Value::is_object) {
            config.insert(section.to_string(), defaults[section].clone());
            changed = true;
        }
    }

    if !config.get("devices").is_some_and(Value::is_array) {
        config.insert("devices".to_string(), json!([]));
        changed = true;
    }

    if !config.get("language").is_some_and(Value::is_string) {
        config.insert("language".to_string(), defaults["language"].clone());
        changed = true;
    }

    let scrcpy = section_mut(&mut config, "scrcpy");
    let presets_fix = match scrcpy.get("presets") {
        Some(Value::Array(presets)) => {
            // Запись без имени не отобразить и не выбрать — она бы просто
            // висела в списке пресетов мусором.
            let valid: Vec<Value> = presets
                .iter()
                .filter(|item| {
                    item.as_object()
                        .and_then(|preset| preset.get("name"))
                        .is_some_and(is_truthy)
                })
                .cloned()
                .collect();
            (valid.len() != presets.len()).then_some(Value::Array(valid))
        }
        _ => Some(default_presets()),
    };
    if let Some(presets) = presets_fix {
        scrcpy.insert("presets".to_string(), presets);
        changed = true;
    }

    let default_port = python_int(&defaults["web"]["port"]).unwrap_or(6969);
    let web = section_mut(&mut config, "web");
    let port = web
        .get("port")
        .and_then(python_int)
        .filter(|port| (1..=65535).contains(port))
        .unwrap_or(default_port);
    if web.get("port") != Some(&Value::from(port)) {
        web.insert("port".to_string(), Value::from(port));
        changed = true;
    }

    if web
        .get("host")
        .and_then(Value::as_str)
        .is_none_or(|host| host.trim().is_empty())
    {
        web.insert("host".to_string(), defaults["web"]["host"].clone());
        changed = true;
    }

    (config, changed)
}

/// Миграция конфига — `mkdsc/config.py::_migrate_config`.
///
/// Возвращает конфиг и признак «что-то поменялось»: по нему `load` решает,
/// нужно ли переписать файл.
fn migrate_config(config: Value, legacy_devices: &Path) -> (Map<String, Value>, bool) {
    let (mut config, mut changed) = sanitize_config(config);
    let defaults = default_config();

    if config.get("config_version") != Some(&Value::from(CONFIG_SCHEMA_VERSION)) {
        config.insert(
            "config_version".to_string(),
            Value::from(CONFIG_SCHEMA_VERSION),
        );
        changed = true;
    }

    let scrcpy = section_mut(&mut config, "scrcpy");
    if !scrcpy.get("presets").is_some_and(is_truthy) {
        scrcpy.insert("presets".to_string(), default_presets());
        changed = true;
    }

    if !config.get("devices").is_some_and(is_truthy) && legacy_devices.exists() {
        if let Some(devices) = read_json_file(legacy_devices)
            .as_ref()
            .and_then(Value::as_object)
            .filter(|legacy| !legacy.is_empty())
            .and_then(|legacy| legacy.get("devices"))
            .filter(|devices| devices.is_array())
        {
            config.insert("devices".to_string(), devices.clone());
            changed = true;
        }
    }

    if !config.contains_key("last_update_check") {
        config.insert("last_update_check".to_string(), Value::Null);
        changed = true;
    }

    for (section, key) in [
        ("logs", "export_dir"),
        ("downloads", "base_dir"),
        ("connection_optimizer", "auto_switch"),
    ] {
        let default = defaults[section][key].clone();
        let target = section_mut(&mut config, section);
        if !target.contains_key(key) {
            target.insert(key.to_string(), default);
            changed = true;
        }
    }

    if let Some(defaults_recording) = defaults["recording"].as_object() {
        let recording = section_mut(&mut config, "recording");
        for (key, value) in defaults_recording {
            if !recording.contains_key(key) {
                recording.insert(key.clone(), value.clone());
                changed = true;
            }
        }
    }

    (config, changed)
}

// ---------------------------------------------------------------------------
// Файл
// ---------------------------------------------------------------------------

/// Читает JSON-файл. Отсутствие, ошибка чтения и битый JSON неразличимы —
/// как `_load_json` в Python.
fn read_json_file(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// `config.json` → `config.json.bak`.
fn backup_path(path: &Path) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => path.with_extension(format!("{ext}.bak")),
        None => path.with_extension("bak"),
    }
}

/// Сохраняет копию нечитаемого конфига вместо того, чтобы молча его затереть.
///
/// Ошибки игнорируются намеренно: не получилось сделать бэкап — приложение всё
/// равно должно подняться на дефолтах.
fn backup_corrupt_config(path: &Path) {
    if let Ok(raw) = std::fs::read(path) {
        let _ = std::fs::write(backup_path(path), raw);
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.json".to_string());
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    // Тот же вид, что у Python (`config.json.<случайное>.tmp`): суффикс `.tmp`
    // ищут тесты, проверяющие, что мусор после записи не остаётся.
    path.with_file_name(format!("{name}.{}-{unique}.tmp", std::process::id()))
}

/// Атомарная запись: обрыв посреди записи не должен обрезать `config.json`.
///
/// Временный файл создаётся в том же каталоге — иначе `rename` через границу
/// файловых систем не сработал бы. `fs::rename` перекрывает существующий файл
/// на обеих целевых платформах (на Windows это `MoveFileExW` с
/// `MOVEFILE_REPLACE_EXISTING`), то есть ведёт себя как `os.replace` в Python;
/// удалять цель заранее не нужно и нельзя — это как раз и открыло бы окно, в
/// котором конфига нет.
fn write_atomic(path: &Path, config: &Value) -> Result<(), ApiError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| ApiError::internal(format!("failed to create config directory: {err}")))?;
    }

    let payload = serde_json::to_string_pretty(config)
        .map_err(|err| ApiError::internal(format!("failed to serialize config: {err}")))?;

    let tmp = temp_path(path);
    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(payload.as_bytes())?;
        file.flush()?;
        file.sync_all()
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(ApiError::internal(format!("failed to write config: {err}")));
    }

    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(ApiError::internal(format!("failed to replace config: {err}")));
    }

    Ok(())
}

/// `datetime.now(timezone.utc).isoformat()` — UTC с микросекундами и смещением
/// `+00:00`, а не `Z`: именно так его записывал Python.
fn utc_now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.6f+00:00")
        .to_string()
}

// ---------------------------------------------------------------------------
// Публичные операции
// ---------------------------------------------------------------------------

fn load_locked(paths: &ConfigPaths) -> Result<Map<String, Value>, ApiError> {
    if !paths.config.exists() {
        let mut config = default_config();
        config.insert(
            "last_update_check".to_string(),
            Value::String(utc_now_iso()),
        );
        write_atomic(&paths.config, &Value::Object(config.clone()))?;
        return Ok(config);
    }

    let loaded = match read_json_file(&paths.config) {
        Some(Value::Object(map)) => map,
        // Нечитаемый файл и файл с не-объектом внутри обрабатываются
        // одинаково: копия в `.bak`, дальше работаем на дефолтах.
        _ => {
            backup_corrupt_config(&paths.config);
            Map::new()
        }
    };

    let merged = deep_merge(&default_config(), &loaded);
    let (merged, changed) = migrate_config(Value::Object(merged), &paths.legacy_devices);

    // Обратная запись на обычном чтении: конфиг, дополненный дефолтами и
    // починенный миграцией, сразу закрепляется в файле. Иначе миграция
    // повторялась бы при каждом запуске.
    if changed || merged != loaded {
        write_atomic(&paths.config, &Value::Object(merged.clone()))?;
    }

    Ok(merged)
}

/// Читает конфиг, дополняя его дефолтами и мигрируя — `load_config` в Python.
pub fn load_at(paths: &ConfigPaths) -> Result<Map<String, Value>, ApiError> {
    let _guard = lock();
    load_locked(paths)
}

/// Частичный патч: `patch` сливается с текущим конфигом.
///
/// Ключ патчем не удалить — для этого есть `replace_at`.
pub fn update_at(
    paths: &ConfigPaths,
    patch: &Map<String, Value>,
) -> Result<Map<String, Value>, ApiError> {
    let _guard = lock();
    let current = load_locked(paths)?;
    let merged = deep_merge(&current, patch);
    let (updated, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
    write_atomic(&paths.config, &Value::Object(updated.clone()))?;
    Ok(updated)
}

/// Полная замена: ключи, которых нет в `new_config`, действительно исчезают.
///
/// Отсутствующие секции добираются из дефолтов, чтобы приложение всё ещё
/// стартовало.
pub fn replace_at(
    paths: &ConfigPaths,
    new_config: &Map<String, Value>,
) -> Result<Map<String, Value>, ApiError> {
    let _guard = lock();
    let merged = deep_merge(&default_config(), new_config);
    let (replaced, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
    write_atomic(&paths.config, &Value::Object(replaced.clone()))?;
    Ok(replaced)
}

/// Читает, даёт изменить и записывает конфиг под одним замком.
///
/// Ради этого замка изменения устройств и пресетов идут именно сюда: два
/// одновременных сохранения иначе прочитали бы одну и ту же версию файла и
/// одно из них пропало бы.
///
/// Миграция после изменения намеренно не запускается — как и `save_config` в
/// Python. Иначе удаление последнего пресета тут же возвращало бы дефолтные:
/// `_migrate_config` считает пустой список пресетов поводом их восстановить.
pub fn mutate_at<F>(paths: &ConfigPaths, apply: F) -> Result<Map<String, Value>, ApiError>
where
    F: FnOnce(&mut Map<String, Value>) -> Result<(), ApiError>,
{
    let _guard = lock();
    let mut config = load_locked(paths)?;
    apply(&mut config)?;
    write_atomic(&paths.config, &Value::Object(config.clone()))?;
    Ok(config)
}

/// См. [`load_at`].
pub fn load(app: &AppHandle) -> Result<Map<String, Value>, ApiError> {
    load_at(&config_paths(app))
}

/// См. [`update_at`].
pub fn update(app: &AppHandle, patch: &Map<String, Value>) -> Result<Map<String, Value>, ApiError> {
    update_at(&config_paths(app), patch)
}

/// См. [`replace_at`].
pub fn replace(
    app: &AppHandle,
    new_config: &Map<String, Value>,
) -> Result<Map<String, Value>, ApiError> {
    replace_at(&config_paths(app), new_config)
}

/// См. [`mutate_at`].
pub fn mutate<F>(app: &AppHandle, apply: F) -> Result<Map<String, Value>, ApiError>
where
    F: FnOnce(&mut Map<String, Value>) -> Result<(), ApiError>,
{
    mutate_at(&config_paths(app), apply)
}

/// Сохранённые устройства из конфига — как есть, без нормализации.
///
/// Форму записей задаёт `save_device` в `api.rs`; фронтенд терпим к лишним
/// полям, но `port` обязан остаться строкой.
pub fn saved_devices(app: &AppHandle) -> Result<Vec<Value>, ApiError> {
    let config = load(app)?;
    Ok(config
        .get("devices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Пресеты scrcpy из уже прочитанного конфига.
pub fn presets_of(config: &Map<String, Value>) -> Value {
    config
        .get("scrcpy")
        .and_then(Value::as_object)
        .and_then(|scrcpy| scrcpy.get("presets"))
        .cloned()
        .unwrap_or_else(|| json!([]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каталог под тесты: `AppHandle` не создать, поэтому работаем с путями.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mkdsc-config-test-{}-{label}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("создать временный каталог");
            Self(path)
        }

        fn paths(&self) -> ConfigPaths {
            ConfigPaths {
                config: self.0.join("config.json"),
                legacy_devices: self.0.join("devices.json"),
            }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("ожидался объект, получено {other}"),
        }
    }

    fn read_back(paths: &ConfigPaths) -> Map<String, Value> {
        object(read_json_file(&paths.config).expect("конфиг читается"))
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri лежит в корне репозитория")
            .to_path_buf()
    }

    // -- порядок ключей и кодировка ----------------------------------------

    /// Главная ловушка вехи: `serde_json` без `preserve_order` отсортировал бы
    /// ключи по алфавиту и перетасовал файл целиком при первой же записи.
    #[test]
    fn writes_config_example_byte_for_byte() {
        let path = repo_root().join("config.example.json");
        let expected = std::fs::read_to_string(&path).expect("config.example.json на месте");
        let parsed: Value = serde_json::from_str(&expected).expect("эталон — валидный JSON");

        let temp = TempDir::new("example");
        let paths = temp.paths();
        write_atomic(&paths.config, &parsed).expect("запись эталона");
        let written = std::fs::read_to_string(&paths.config).expect("читаем записанное");

        // Единственное расхождение с эталоном — завершающий перевод строки:
        // `json.dumps` его не пишет, а в репозитории файл лежит с ним.
        assert_eq!(written, expected.trim_end_matches('\n'));
    }

    /// `ensure_ascii=False`: кириллица в путях и именах устройств остаётся
    /// кириллицей, а не превращается в `\uXXXX`.
    #[test]
    fn keeps_non_ascii_as_is() {
        let temp = TempDir::new("unicode");
        let paths = temp.paths();

        let mut config = default_config();
        config.insert(
            "downloads".to_string(),
            json!({"base_dir": "D:/Загрузки/Видео"}),
        );
        write_atomic(&paths.config, &Value::Object(config)).expect("запись");

        let raw = std::fs::read_to_string(&paths.config).expect("конфиг читается");
        assert!(raw.contains("D:/Загрузки/Видео"), "записано: {raw}");
        assert!(!raw.contains("\\u04"), "экранированная кириллица: {raw}");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["downloads"]["base_dir"], json!("D:/Загрузки/Видео"));
    }

    /// Порядок ключей не должен меняться от того, что конфиг перезаписали.
    #[test]
    fn rewrite_keeps_key_order() {
        let temp = TempDir::new("order");
        let paths = temp.paths();

        let mut created = load_at(&paths).expect("конфиг создаётся");
        let before: Vec<String> = created.keys().cloned().collect();

        created.insert("language".to_string(), json!("ru"));
        write_atomic(&paths.config, &Value::Object(created)).expect("перезапись");

        let after: Vec<String> = read_back(&paths).keys().cloned().collect();
        assert_eq!(before, after);
        assert_eq!(before.first().map(String::as_str), Some("config_version"));
    }

    // -- порт `tests/test_config.py` ---------------------------------------

    /// `test_default_host_is_loopback`.
    #[test]
    fn default_host_is_loopback() {
        assert_eq!(default_config()["web"]["host"], json!("127.0.0.1"));
    }

    /// `test_migrate_survives_wrong_types` — BUG-09: одна `null`-секция
    /// заставляла каждый эндпоинт возвращать 500 навсегда.
    #[test]
    fn migrate_survives_wrong_types() {
        let payloads = [
            json!({"scrcpy": null}),
            json!({"web": 5}),
            json!({"logs": []}),
            json!({"recording": "x"}),
            json!({"devices": 3}),
            json!({"connection_optimizer": null}),
        ];

        let temp = TempDir::new("wrong-types");
        let paths = temp.paths();

        for payload in payloads {
            let merged = deep_merge(&default_config(), &object(payload.clone()));
            let (result, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
            assert!(result["scrcpy"].is_object(), "случай: {payload}");
            assert!(result["web"].is_object(), "случай: {payload}");
            assert!(result["devices"].is_array(), "случай: {payload}");
        }
    }

    /// `test_load_config_recovers_from_null_section`.
    #[test]
    fn load_recovers_from_null_section() {
        let temp = TempDir::new("null-section");
        let paths = temp.paths();

        let mut broken = default_config();
        broken.insert("scrcpy".to_string(), Value::Null);
        write_atomic(&paths.config, &Value::Object(broken)).expect("запись битого конфига");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert!(loaded["scrcpy"].is_object());
        assert!(is_truthy(&loaded["scrcpy"]["presets"]));
    }

    /// `test_load_config_backs_up_corrupt_file`.
    #[test]
    fn load_backs_up_corrupt_file() {
        let temp = TempDir::new("corrupt");
        let paths = temp.paths();
        std::fs::write(&paths.config, "{not json").expect("пишем мусор");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["web"]["port"], json!(6969));
        assert!(backup_path(&paths.config).exists());
    }

    /// `test_update_config_merges_but_cannot_delete` — BUG-10: POST это патч,
    /// удалять им нельзя.
    #[test]
    fn update_merges_but_cannot_delete() {
        let temp = TempDir::new("patch");
        let paths = temp.paths();

        update_at(&paths, &object(json!({"scrcpy": {"custom": "x"}}))).expect("первый патч");
        update_at(&paths, &object(json!({"scrcpy": {}}))).expect("пустой патч");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["scrcpy"]["custom"], json!("x"));
    }

    /// Одновременные изменения не должны терять друг друга.
    ///
    /// Ровно ради этого `mutate_at` держит замок на всю пару «чтение —
    /// запись»: иначе сохранение пресета и сохранение устройства прочитали бы
    /// одну и ту же версию файла, и одна из правок исчезла бы.
    #[test]
    fn concurrent_mutations_all_survive() {
        let temp = TempDir::new("concurrent");
        let paths = temp.paths();
        load_at(&paths).expect("конфиг создаётся");

        std::thread::scope(|scope| {
            for index in 0..8 {
                let paths = &paths;
                scope.spawn(move || {
                    mutate_at(paths, |config| {
                        let devices = config
                            .get_mut("devices")
                            .and_then(Value::as_array_mut)
                            .expect("список устройств");
                        devices.push(json!({"name": format!("device-{index}")}));
                        Ok(())
                    })
                    .expect("изменение применяется");
                });
            }
        });

        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["devices"].as_array().map(Vec::len), Some(8));
    }

    /// `test_replace_config_deletes_removed_keys`.
    #[test]
    fn replace_deletes_removed_keys() {
        let temp = TempDir::new("replace");
        let paths = temp.paths();

        update_at(&paths, &object(json!({"scrcpy": {"custom": "x"}}))).expect("патч");
        let mut full = load_at(&paths).expect("конфиг читается");
        let scrcpy = section_mut(&mut full, "scrcpy");
        // `shift_remove`, а не `remove`: при `preserve_order` последний
        // переставляет ключи местами и порядок в файле поехал бы.
        scrcpy.shift_remove("custom");

        replace_at(&paths, &full).expect("полная замена");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert!(!loaded["scrcpy"].as_object().unwrap().contains_key("custom"));
    }

    /// `test_save_config_is_atomic` — BUG-11: между усечением и записью не
    /// должно быть окна, в котором файл наполовину пуст.
    #[test]
    fn save_is_atomic() {
        let temp = TempDir::new("atomic");
        let paths = temp.paths();

        write_atomic(&paths.config, &Value::Object(default_config())).expect("первая запись");
        let original = std::fs::read_to_string(&paths.config).expect("читаем первую версию");

        let mut updated = default_config();
        updated.insert("language".to_string(), json!("ru"));
        write_atomic(&paths.config, &Value::Object(updated)).expect("вторая запись");

        assert_eq!(read_back(&paths)["language"], json!("ru"));
        assert_eq!(
            serde_json::from_str::<Value>(&original).unwrap()["language"],
            json!("en")
        );

        let leftovers: Vec<PathBuf> = std::fs::read_dir(&temp.0)
            .expect("читаем каталог")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "остались временные файлы: {leftovers:?}");
    }

    /// `test_invalid_port_falls_back`.
    #[test]
    fn invalid_port_falls_back() {
        let temp = TempDir::new("port");
        let paths = temp.paths();
        let merged = deep_merge(&default_config(), &object(json!({"web": {"port": "banana"}})));
        let (result, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
        assert_eq!(result["web"]["port"], json!(6969));
    }

    // -- дополнительно ------------------------------------------------------

    /// Порт-строка приводится к числу, порт вне диапазона — к дефолтному.
    #[test]
    fn port_is_normalized() {
        let temp = TempDir::new("port-range");
        let paths = temp.paths();
        let cases = [
            (json!("7000"), json!(7000)),
            (json!(0), json!(6969)),
            (json!(70000), json!(6969)),
            (json!(null), json!(6969)),
            (json!(8080), json!(8080)),
        ];
        for (input, expected) in cases {
            let merged = deep_merge(
                &default_config(),
                &object(json!({"web": {"port": input.clone()}})),
            );
            let (result, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
            assert_eq!(result["web"]["port"], expected, "порт: {input}");
        }
    }

    /// Пресеты без имени выбрасываются, пустой список восстанавливается.
    #[test]
    fn presets_are_repaired() {
        let temp = TempDir::new("presets");
        let paths = temp.paths();

        let merged = deep_merge(
            &default_config(),
            &object(json!({"scrcpy": {"presets": [{"name": "Mine"}, {"bitrate": "8M"}, "junk"]}})),
        );
        let (result, changed) = migrate_config(Value::Object(merged), &paths.legacy_devices);
        assert!(changed);
        assert_eq!(result["scrcpy"]["presets"], json!([{"name": "Mine"}]));

        let merged = deep_merge(
            &default_config(),
            &object(json!({"scrcpy": {"presets": []}})),
        );
        let (result, _) = migrate_config(Value::Object(merged), &paths.legacy_devices);
        assert_eq!(result["scrcpy"]["presets"], default_presets());
    }

    /// Устройства из `devices.json` подхватываются, пока список в конфиге пуст.
    ///
    /// Именно при существующем `config.json`: ветка первого запуска в Python
    /// возвращает дефолты, не заходя в миграцию, и legacy-файл там не читается.
    /// Поведение воспроизведено как есть — расхождение означало бы, что Rust и
    /// Python видят разные списки устройств.
    #[test]
    fn legacy_devices_are_imported() {
        let temp = TempDir::new("legacy");
        let paths = temp.paths();
        std::fs::write(
            &paths.legacy_devices,
            r#"{"devices": [{"name": "Old", "ip": "10.0.0.2", "port": "5555"}]}"#,
        )
        .expect("пишем legacy-файл");
        std::fs::write(&paths.config, "{}").expect("пишем пустой конфиг");

        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["devices"][0]["name"], json!("Old"));

        // Второй раз импорт не повторяется: список уже не пуст.
        std::fs::write(&paths.legacy_devices, r#"{"devices": [{"name": "Newer"}]}"#)
            .expect("переписываем legacy-файл");
        let loaded = load_at(&paths).expect("конфиг читается");
        assert_eq!(loaded["devices"][0]["name"], json!("Old"));
    }

    /// Повторное чтение не переписывает файл: иначе каждый опрос устройств
    /// дёргал бы диск, а `git diff` шумел бы на ровном месте.
    #[test]
    fn second_load_is_stable() {
        let temp = TempDir::new("stable");
        let paths = temp.paths();

        load_at(&paths).expect("первое чтение");
        let first = std::fs::read_to_string(&paths.config).expect("читаем файл");
        let modified = std::fs::metadata(&paths.config)
            .and_then(|meta| meta.modified())
            .expect("время изменения");

        load_at(&paths).expect("второе чтение");
        let second = std::fs::read_to_string(&paths.config).expect("читаем файл");
        let modified_again = std::fs::metadata(&paths.config)
            .and_then(|meta| meta.modified())
            .expect("время изменения");

        assert_eq!(first, second);
        assert_eq!(modified, modified_again);
    }

    /// Слияние списков — замена целиком, а не поэлементная склейка.
    #[test]
    fn deep_merge_replaces_lists() {
        let merged = deep_merge(
            &object(json!({"a": {"b": 1, "c": 2}, "list": [1, 2, 3]})),
            &object(json!({"a": {"c": 9}, "list": [7], "extra": true})),
        );
        assert_eq!(
            Value::Object(merged),
            json!({"a": {"b": 1, "c": 9}, "list": [7], "extra": true})
        );
    }

    /// Версия в `Cargo.toml` уходит в `GET /api/config`; она обязана совпадать
    /// с `mkdsc/constants.py::VERSION`, иначе UI покажет чужой номер.
    #[test]
    fn version_matches_python_constant() {
        let source = std::fs::read_to_string(repo_root().join("mkdsc").join("constants.py"))
            .expect("mkdsc/constants.py на месте");
        let python_version = source
            .lines()
            .find_map(|line| line.strip_prefix("VERSION = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .expect("VERSION найден");
        assert_eq!(python_version, env!("CARGO_PKG_VERSION"));
    }

    /// То же для версии схемы конфига.
    #[test]
    fn schema_version_matches_python_constant() {
        let source = std::fs::read_to_string(repo_root().join("mkdsc").join("constants.py"))
            .expect("mkdsc/constants.py на месте");
        let python_version = source
            .lines()
            .find_map(|line| line.strip_prefix("CONFIG_SCHEMA_VERSION = "))
            .and_then(|value| value.trim().parse::<i64>().ok())
            .expect("CONFIG_SCHEMA_VERSION найден");
        assert_eq!(python_version, CONFIG_SCHEMA_VERSION);
    }
}
