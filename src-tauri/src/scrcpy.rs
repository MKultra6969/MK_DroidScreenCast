//! Запуск scrcpy и общая машинерия вокруг него.
//!
//! Питоновские оригиналы — `launch_scrcpy_api`, `_apply_device_settings`,
//! `_restore_device_settings` и `_normalize_keyboard_mode` в
//! `mkdsc/web/server.py`.
//!
//! Набор аргументов scrcpy переносится дословно: он выверен на живых
//! устройствах, и «улучшать» его в рамках порта нельзя.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};
use tokio::process::{Child, Command};

use crate::error::ApiError;
use crate::{config, devices};

/// AOA на Windows не работает — фронтенд показывает это предупреждение.
pub const AOA_WINDOWS_FALLBACK: &str = "notification_aoa_windows_fallback";

/// Сколько всего готовы ждать восстановления настроек при выходе.
///
/// Каждая команда adb ограничена своим таймаутом, но на выходе из приложения
/// даже одна зависшая пауза заметна: окно уже закрылось, а процесс ещё жив.
/// Лучше не вернуть настройку, чем подвесить закрытие.
const EXIT_RESTORE_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Настройки устройства
// ---------------------------------------------------------------------------

/// Настройка, которую приложение меняет на время сеанса.
struct DeviceSetting {
    namespace: &'static str,
    key: &'static str,
    value: &'static str,
    /// Имя для `failed_settings` в ответе — фронтенд показывает его как есть.
    label: &'static str,
}

/// «Не гасить экран». 3 = от сети и от USB одновременно.
const STAY_AWAKE: DeviceSetting = DeviceSetting {
    namespace: "global",
    key: "stay_on_while_plugged_in",
    value: "3",
    label: "stay_awake",
};

/// «Показывать касания».
const SHOW_TOUCHES: DeviceSetting = DeviceSetting {
    namespace: "system",
    key: "show_touches",
    value: "1",
    label: "show_touches",
};

/// Что вернуть устройству после закрытия scrcpy.
///
/// `None` в значении означает, что настройки не было вовсе, — такую надо
/// удалять, а не записывать пустой строкой.
#[derive(Debug, Default, Clone)]
pub struct Restore {
    entries: Vec<(&'static str, &'static str, Option<String>)>,
}

impl Restore {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Применяет настройки устройства и запоминает прежние значения.
///
/// Возвращает карту восстановления и список настроек, которые применить не
/// удалось, — он уезжает в ответ как `failed_settings`.
///
/// **Серийник обязателен.** Раньше его не передавали, и при двух подключённых
/// устройствах adb отвечал «more than one device»: настройки молча не
/// применялись, а `stay_on_while_plugged_in=3` мог навсегда остаться включённым
/// на телефоне.
pub async fn apply_device_settings(
    adb: &Path,
    stay_awake: bool,
    show_touches: bool,
    serial: Option<&str>,
) -> Result<(Restore, Vec<String>), ApiError> {
    let mut restore = Restore::default();
    let mut failed = Vec::new();

    for (wanted, setting) in [(stay_awake, STAY_AWAKE), (show_touches, SHOW_TOUCHES)] {
        if !wanted {
            continue;
        }

        let previous = devices::get_setting(adb, setting.namespace, setting.key, serial).await?;
        if devices::put_setting(adb, setting.namespace, setting.key, setting.value, serial).await? {
            restore
                .entries
                .push((setting.namespace, setting.key, previous));
        } else {
            failed.push(setting.label.to_string());
        }
    }

    Ok((restore, failed))
}

/// Возвращает настройки устройства как было.
///
/// Ошибки проглатываются: восстановление идёт в фоне, показать их некому, а
/// прерванная на полпути функция оставила бы вторую настройку изменённой.
pub async fn restore_device_settings(adb: &Path, restore: &Restore, serial: Option<&str>) {
    for (namespace, key, previous) in &restore.entries {
        let _ = match previous {
            Some(value) => devices::put_setting(adb, namespace, key, value, serial).await,
            None => devices::delete_setting(adb, namespace, key, serial).await,
        };
    }
}

// ---------------------------------------------------------------------------
// Очередь восстановлений
// ---------------------------------------------------------------------------

/// Восстановление, которое ещё не выполнено.
struct Pending {
    id: u64,
    adb: PathBuf,
    serial: Option<String>,
    restore: Restore,
}

/// Незакрытые восстановления.
///
/// Фоновой задачи, ждущей выхода scrcpy, мало: она умирает вместе с рантаймом,
/// и если пользователь закроет приложение при живом scrcpy, настройки останутся
/// изменёнными на телефоне. Очередь дочищается в обработчике выхода
/// ([`restore_on_exit`]).
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

/// Ставит восстановление в очередь. `None` — восстанавливать нечего.
fn remember(adb: &Path, serial: Option<&str>, restore: Restore) -> Option<u64> {
    if restore.is_empty() {
        return None;
    }

    let id = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut pending) = PENDING.lock() {
        pending.push(Pending {
            id,
            adb: adb.to_path_buf(),
            serial: serial.map(str::to_string),
            restore,
        });
    }
    Some(id)
}

/// Забирает восстановление из очереди — больше его никто не выполнит.
fn take(id: u64) -> Option<Pending> {
    let mut pending = PENDING.lock().ok()?;
    let index = pending.iter().position(|entry| entry.id == id)?;
    Some(pending.remove(index))
}

/// Выполняет отложенное восстановление, если его ещё не забрал выход из
/// приложения.
pub async fn run_pending(id: u64) {
    let Some(entry) = take(id) else {
        return;
    };
    restore_device_settings(&entry.adb, &entry.restore, entry.serial.as_deref()).await;
}

/// Возвращает настройки всем устройствам, для которых scrcpy ещё жив.
///
/// Вызывается из обработчика выхода Tauri, поэтому блокирующая: рантайм уже
/// сворачивается, откладывать в фон некуда.
pub fn restore_on_exit() {
    let entries: Vec<Pending> = match PENDING.lock() {
        Ok(mut pending) => pending.drain(..).collect(),
        Err(_) => return,
    };
    if entries.is_empty() {
        return;
    }

    tauri::async_runtime::block_on(async move {
        let _ = tokio::time::timeout(EXIT_RESTORE_BUDGET, async {
            for entry in entries {
                restore_device_settings(&entry.adb, &entry.restore, entry.serial.as_deref()).await;
            }
        })
        .await;
    });
}

// ---------------------------------------------------------------------------
// Разбор запроса
// ---------------------------------------------------------------------------

/// Нормализует режим клавиатуры.
///
/// AOA требует эксклюзивного доступа к USB-устройству, которого на Windows нет
/// без подмены драйвера, — там режим заменяется на UHID, а наружу уезжает
/// предупреждение.
pub fn normalize_keyboard_mode(keyboard: &str) -> (String, Option<&'static str>) {
    if keyboard == "aoa" && cfg!(windows) {
        ("uhid".to_string(), Some(AOA_WINDOWS_FALLBACK))
    } else {
        (keyboard.to_string(), None)
    }
}

/// Приводит пользовательский префикс к безопасному имени файла.
///
/// Всё, кроме латиницы, цифр, `-` и `_`, становится подчёркиванием; подряд
/// идущие подчёркивания схлопываются, крайние отбрасываются. Пустой результат
/// превращается в `recording`.
pub fn sanitize_prefix(prefix: &str) -> String {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return "recording".to_string();
    }

    let mut cleaned = String::with_capacity(prefix.len());
    for ch in prefix.chars() {
        let allowed = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_';
        let ch = if allowed { ch } else { '_' };
        // Схлопывание на месте — `re.sub(r"_+", "_", ...)` в Python.
        if ch == '_' && cleaned.ends_with('_') {
            continue;
        }
        cleaned.push(ch);
    }

    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "recording".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Метка времени в имени файла — `datetime.now().strftime("%Y%m%d_%H%M%S")`.
pub fn file_timestamp() -> String {
    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
}

/// Строковое поле тела запроса, если оно «истинно» по правилам Python.
fn truthy_str(data: &Map<String, Value>, key: &str) -> Option<String> {
    data.get(key)
        .filter(|value| config::is_truthy(value))
        .map(config::python_str)
}

/// Булево поле тела запроса — `bool(data.get(key))`.
fn flag(data: &Map<String, Value>, key: &str) -> bool {
    data.get(key).is_some_and(config::is_truthy)
}

/// Секция конфига как объект.
fn section<'a>(config: &'a Map<String, Value>, name: &str) -> Option<&'a Map<String, Value>> {
    config.get(name)?.as_object()
}

/// Значение из тела запроса, иначе из секции конфига.
fn truthy_or_config(
    data: &Map<String, Value>,
    cfg: Option<&Map<String, Value>>,
    key: &str,
) -> Option<String> {
    truthy_str(data, key).or_else(|| {
        cfg?.get(key)
            .filter(|value| config::is_truthy(value))
            .map(config::python_str)
    })
}

/// Флаг из тела запроса, иначе из конфига.
///
/// Важно именно наличие ключа в теле, а не его истинность: снятая
/// пользователем галочка обязана перебивать `true` из конфига, поэтому здесь
/// `contains_key`, а не проверка значения.
fn flag_or_config(data: &Map<String, Value>, cfg: Option<&Map<String, Value>>, key: &str) -> bool {
    if data.contains_key(key) {
        return flag(data, key);
    }
    cfg.and_then(|cfg| cfg.get(key))
        .is_some_and(config::is_truthy)
}

/// Аргументы выбора устройства: `--serial` либо `--select-usb`/`--select-tcpip`.
fn device_args(serial: Option<&str>, connection: Option<&str>) -> Vec<String> {
    match serial {
        Some(serial) => vec!["--serial".to_string(), serial.to_string()],
        None => match connection {
            Some("usb") => vec!["--select-usb".to_string()],
            Some("wifi") => vec!["--select-tcpip".to_string()],
            _ => Vec::new(),
        },
    }
}

/// Разобранный запрос на просмотр — `POST /api/scrcpy/launch`.
pub struct LaunchPlan {
    pub args: Vec<String>,
    pub warning_key: Option<&'static str>,
    pub serial: Option<String>,
    pub stay_awake: bool,
    pub show_touches: bool,
}

impl LaunchPlan {
    /// Собирает аргументы. Конфиг здесь не читается — как и в Python, всё
    /// нужное приходит в теле запроса.
    pub fn resolve(data: &Map<String, Value>) -> Self {
        let mut args = Vec::new();

        if let Some(bitrate) = truthy_str(data, "bitrate") {
            args.push("--video-bit-rate".to_string());
            args.push(bitrate);
        }
        if let Some(maxsize) = truthy_str(data, "maxsize") {
            args.push("--max-size".to_string());
            args.push(maxsize);
        }

        // `data.get("keyboard", "uhid")`: подставляется только отсутствующий
        // ключ, пустая строка уезжает как есть — как в Python.
        let keyboard = data
            .get("keyboard")
            .map_or_else(|| "uhid".to_string(), config::python_str);
        let (keyboard, warning_key) = normalize_keyboard_mode(&keyboard);
        args.push(format!("--keyboard={keyboard}"));

        let serial = truthy_str(data, "serial");
        let connection = truthy_str(data, "connection");
        args.extend(device_args(serial.as_deref(), connection.as_deref()));

        if flag(data, "turn_screen_off") {
            args.push("--turn-screen-off".to_string());
        }
        if flag(data, "fullscreen") {
            args.push("--fullscreen".to_string());
        }
        if flag(data, "no_audio") {
            args.push("--no-audio".to_string());
        }

        Self {
            args,
            warning_key,
            serial,
            stay_awake: flag(data, "stay_awake"),
            show_touches: flag(data, "show_touches"),
        }
    }
}

/// Разобранный запрос на запись — тело, дополненное конфигом.
#[derive(Debug)]
pub struct RecordingPlan {
    pub output_dir: PathBuf,
    pub format: String,
    pub prefix: String,
    pub warning_key: Option<&'static str>,
    pub serial: Option<String>,
    pub connection: Option<String>,
    pub audio_source: String,
    pub show_preview: bool,
    pub stay_awake: bool,
    pub show_touches: bool,
    /// Всё, что не зависит от пути к файлу; путь дописывается в [`Self::args`].
    tail: Vec<String>,
}

impl RecordingPlan {
    /// Разбирает тело запроса и конфиг.
    ///
    /// `default_dir` — каталог записей из конфига; он же используется, когда
    /// каталога нет ни в теле, ни в `recording.output_dir`.
    ///
    /// 400, если формат не `mp4` и не `mkv`.
    pub fn resolve(
        data: &Map<String, Value>,
        config: &Map<String, Value>,
        default_dir: PathBuf,
    ) -> Result<Self, ApiError> {
        let recording_cfg = section(config, "recording");
        let scrcpy_cfg = section(config, "scrcpy");

        let output_dir = truthy_or_config(data, recording_cfg, "output_dir")
            .map_or(default_dir, |dir| crate::paths::expand_user(&dir));

        let format = truthy_or_config(data, recording_cfg, "format")
            .unwrap_or_else(|| "mp4".to_string())
            .to_lowercase();
        if format != "mp4" && format != "mkv" {
            return Err(ApiError::new(400, "Unsupported format"));
        }

        let prefix = sanitize_prefix(
            &truthy_or_config(data, recording_cfg, "file_prefix").unwrap_or_default(),
        );

        let mut tail = Vec::new();
        // Дефолты повторяют `config.get("scrcpy", {}).get(..., "8M")` в Python:
        // они подставляются, даже когда секции scrcpy в конфиге нет вовсе.
        let bitrate = truthy_or_config(data, scrcpy_cfg, "bitrate").unwrap_or_else(|| "8M".into());
        tail.push("--video-bit-rate".to_string());
        tail.push(bitrate);

        let maxsize = truthy_or_config(data, scrcpy_cfg, "maxsize").unwrap_or_else(|| "1080".into());
        tail.push("--max-size".to_string());
        tail.push(maxsize);

        let keyboard =
            truthy_or_config(data, scrcpy_cfg, "keyboard").unwrap_or_else(|| "uhid".to_string());
        let (keyboard, warning_key) = normalize_keyboard_mode(&keyboard);
        tail.push(format!("--keyboard={keyboard}"));

        let serial = truthy_str(data, "serial");
        let connection = truthy_str(data, "connection");
        tail.extend(device_args(serial.as_deref(), connection.as_deref()));

        if flag_or_config(data, recording_cfg, "turn_screen_off") {
            tail.push("--turn-screen-off".to_string());
        }

        let show_preview = match data.get("show_preview") {
            // `null` в теле — это «не задано», как `is None` в Python.
            Some(Value::Null) | None => recording_cfg
                .and_then(|cfg| cfg.get("show_preview"))
                .is_none_or(config::is_truthy),
            Some(value) => config::is_truthy(value),
        };
        if !show_preview {
            tail.push("--no-window".to_string());
            tail.push("--no-audio-playback".to_string());
        }

        let audio_source = truthy_or_config(data, recording_cfg, "audio_source")
            .unwrap_or_else(|| "output".to_string())
            .to_lowercase();
        if audio_source == "none" || audio_source == "off" {
            tail.push("--no-audio".to_string());
        } else {
            tail.push(format!("--audio-source={audio_source}"));
        }

        Ok(Self {
            output_dir,
            format,
            prefix,
            warning_key,
            serial,
            connection,
            audio_source,
            show_preview,
            stay_awake: flag_or_config(data, recording_cfg, "stay_awake"),
            show_touches: flag_or_config(data, recording_cfg, "show_touches"),
            tail,
        })
    }

    /// Имя файла записи: `{префикс}_{время}.{формат}`.
    pub fn file_name(&self, timestamp: &str) -> String {
        format!("{}_{}.{}", self.prefix, timestamp, self.format)
    }

    /// Полный список аргументов scrcpy.
    pub fn args(&self, output_path: &Path) -> Vec<String> {
        let mut args = vec![
            "--record".to_string(),
            output_path.to_string_lossy().into_owned(),
        ];
        args.extend(self.tail.iter().cloned());
        args
    }

    /// Срез настроек для ответа и статуса — форма из `_recording_status_payload`.
    pub fn settings(&self) -> Value {
        serde_json::json!({
            "format": self.format,
            "audio_source": self.audio_source,
            "show_preview": self.show_preview,
            "serial": self.serial,
            "connection": self.connection,
        })
    }
}

// ---------------------------------------------------------------------------
// Запуск процесса
// ---------------------------------------------------------------------------

/// Строка команды для ответа — `" ".join(cmd)` в Python.
///
/// Именно строка, а не список: её показывает интерфейс, и кавычек Python туда
/// тоже не добавляет.
pub fn command_line(program: &Path, args: &[String]) -> String {
    let mut parts = vec![program.to_string_lossy().into_owned()];
    parts.extend(args.iter().cloned());
    parts.join(" ")
}

/// Запускает scrcpy для просмотра — процесс живёт сам по себе.
///
/// Вывод не перехватывается: он никому не нужен, а лишняя труба — это ещё один
/// способ подвесить процесс, заполнивший её буфер.
pub fn spawn_viewer(scrcpy: &Path, args: &[String]) -> Result<Child, ApiError> {
    let mut command = Command::new(scrcpy);
    command.args(args).stdin(Stdio::null());
    // Без этого у scrcpy появилось бы собственное окно консоли: своей консоли у
    // лаунчера нет, и Windows завела бы потомку новую — с окном.
    crate::set_no_window(command.as_std_mut());
    command
        .spawn()
        .map_err(|err| ApiError::internal(format!("failed to run scrcpy: {err}")))
}

/// Запускает scrcpy на запись.
///
/// Вывод уходит в трубы: он нужен для `last_error`, и вычитывать его
/// обязательно — процесс, заполнивший буфер трубы, встанет намертво.
///
/// Флаги создания процесса на Windows выбраны так, чтобы работала мягкая
/// остановка; почему именно эти — в `signal.rs`.
pub fn spawn_recorder(scrcpy: &Path, args: &[String]) -> Result<Child, ApiError> {
    let mut command = Command::new(scrcpy);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    set_recorder_flags(command.as_std_mut());
    command
        .spawn()
        .map_err(|err| ApiError::internal(format!("failed to run scrcpy: {err}")))
}

#[cfg(windows)]
fn set_recorder_flags(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    /// Своя безоконная консоль — в неё подсаживается `signal::request_stop`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// Своя группа процессов: `CTRL_BREAK` достанется только scrcpy.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    // Одним вызовом: `creation_flags` флаги заменяет, а не добавляет, и второй
    // вызов (например `set_no_window`) стёр бы группу.
    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
fn set_recorder_flags(_command: &mut std::process::Command) {}

/// Ставит восстановление настроек в очередь, не забирая процесс.
///
/// Нужно записи: там за процессом следит своя задача, она же закроет
/// восстановление по выданному номеру.
pub fn defer_restore(adb: &Path, serial: Option<&str>, restore: Restore) -> Option<u64> {
    remember(adb, serial, restore)
}

/// Ждёт завершения scrcpy и возвращает настройки устройства.
pub fn restore_when_finished(adb: &Path, serial: Option<&str>, restore: Restore, mut child: Child) {
    let ticket = remember(adb, serial, restore);

    // Процесс дожидаемся в любом случае, даже когда возвращать нечего: иначе на
    // POSIX он остался бы зомби до выхода из приложения.
    tauri::async_runtime::spawn(async move {
        let _ = child.wait().await;
        if let Some(ticket) = ticket {
            run_pending(ticket).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().expect("объект").clone()
    }

    #[test]
    fn normalizes_keyboard_only_on_windows() {
        let (mode, warning) = normalize_keyboard_mode("aoa");
        if cfg!(windows) {
            assert_eq!(mode, "uhid");
            assert_eq!(warning, Some(AOA_WINDOWS_FALLBACK));
        } else {
            assert_eq!(mode, "aoa");
            assert_eq!(warning, None);
        }

        // Остальные режимы не трогаются ни на одной платформе.
        for mode in ["uhid", "sdk", "disabled"] {
            assert_eq!(normalize_keyboard_mode(mode), (mode.to_string(), None));
        }
    }

    #[test]
    fn launch_args_mirror_python() {
        let plan = LaunchPlan::resolve(&object(json!({
            "bitrate": "8M",
            "maxsize": "1080",
            "keyboard": "uhid",
            "serial": "R58M123",
            "turn_screen_off": true,
            "fullscreen": true,
            "no_audio": true,
            "stay_awake": true,
            "show_touches": true,
        })));

        assert_eq!(
            plan.args,
            [
                "--video-bit-rate",
                "8M",
                "--max-size",
                "1080",
                "--keyboard=uhid",
                "--serial",
                "R58M123",
                "--turn-screen-off",
                "--fullscreen",
                "--no-audio",
            ]
        );
        assert_eq!(plan.serial.as_deref(), Some("R58M123"));
        assert!(plan.stay_awake && plan.show_touches);
    }

    #[test]
    fn launch_falls_back_to_connection_when_serial_is_missing() {
        for (connection, expected) in [("usb", "--select-usb"), ("wifi", "--select-tcpip")] {
            let plan = LaunchPlan::resolve(&object(json!({"connection": connection})));
            assert!(plan.args.contains(&expected.to_string()), "{connection}");
        }

        // Серийник главнее: при нём выбор по типу подключения не нужен.
        let plan = LaunchPlan::resolve(&object(json!({"serial": "X", "connection": "usb"})));
        assert!(!plan.args.iter().any(|arg| arg.starts_with("--select")));

        // Неизвестный тип — просто ничего, как в Python.
        let plan = LaunchPlan::resolve(&object(json!({"connection": "carrier-pigeon"})));
        assert_eq!(plan.args, ["--keyboard=uhid"]);
    }

    #[test]
    fn launch_keeps_python_defaults_for_missing_keys() {
        // Пустое тело: только режим клавиатуры по умолчанию.
        let plan = LaunchPlan::resolve(&Map::new());
        assert_eq!(plan.args, ["--keyboard=uhid"]);
        assert!(!plan.stay_awake);

        // Пустые строки — ложь в Python, флаг не добавляется.
        let plan = LaunchPlan::resolve(&object(json!({"bitrate": "", "maxsize": ""})));
        assert_eq!(plan.args, ["--keyboard=uhid"]);
    }

    #[test]
    fn sanitizes_prefix_like_python() {
        let cases = [
            ("clip", "clip"),
            ("  clip  ", "clip"),
            ("", "recording"),
            ("   ", "recording"),
            ("my clip", "my_clip"),
            ("my   clip", "my_clip"),
            ("../../etc/passwd", "etc_passwd"),
            ("_leading", "leading"),
            ("trailing_", "trailing"),
            ("___", "recording"),
            ("!!!", "recording"),
            ("a-b_c1", "a-b_c1"),
            // Кириллица не проходит проверку `"a" <= ch <= "z"` в Python и
            // становится подчёркиваниями — здесь ровно то же самое.
            ("запись", "recording"),
            ("клип2", "2"),
            ("C:\\dir\\file", "C_dir_file"),
        ];

        for (input, expected) in cases {
            assert_eq!(sanitize_prefix(input), expected, "вход: {input:?}");
        }
    }

    fn recording_plan(data: Value, config: Value) -> RecordingPlan {
        RecordingPlan::resolve(
            &object(data),
            &object(config),
            PathBuf::from("/default/recordings"),
        )
        .expect("план собрался")
    }

    #[test]
    fn recording_args_mirror_python() {
        let plan = recording_plan(
            json!({"serial": "R58M123", "format": "mkv", "audio_source": "mic"}),
            json!({"scrcpy": {"bitrate": "16M", "maxsize": "1440", "keyboard": "uhid"}}),
        );

        assert_eq!(
            plan.args(Path::new("/tmp/clip.mkv")),
            [
                "--record",
                "/tmp/clip.mkv",
                "--video-bit-rate",
                "16M",
                "--max-size",
                "1440",
                "--keyboard=uhid",
                "--serial",
                "R58M123",
                "--audio-source=mic",
            ]
        );
        assert_eq!(plan.format, "mkv");
    }

    #[test]
    fn recording_uses_config_defaults() {
        let plan = recording_plan(json!({}), json!({}));
        let args = plan.args(Path::new("/tmp/clip.mp4"));

        assert!(args.contains(&"8M".to_string()));
        assert!(args.contains(&"1080".to_string()));
        assert!(args.contains(&"--keyboard=uhid".to_string()));
        assert!(args.contains(&"--audio-source=output".to_string()));
        assert_eq!(plan.output_dir, PathBuf::from("/default/recordings"));
        assert_eq!(plan.format, "mp4");
        assert_eq!(plan.prefix, "recording");
        assert!(plan.show_preview, "по умолчанию окно показывается");
    }

    #[test]
    fn recording_without_preview_adds_headless_flags() {
        let plan = recording_plan(json!({"show_preview": false}), json!({}));
        let args = plan.args(Path::new("/tmp/clip.mp4"));

        assert!(args.contains(&"--no-window".to_string()));
        assert!(args.contains(&"--no-audio-playback".to_string()));
        assert!(!plan.show_preview);
    }

    #[test]
    fn recording_body_flag_overrides_config() {
        // Явный `false` в теле обязан перебивать `true` из конфига: иначе
        // снятая галочка «не гасить экран» молча не срабатывала бы.
        let plan = recording_plan(
            json!({"stay_awake": false, "show_preview": false}),
            json!({"recording": {"stay_awake": true, "show_preview": true}}),
        );
        assert!(!plan.stay_awake);
        assert!(!plan.show_preview);

        // Отсутствие ключа, наоборот, отдаёт решение конфигу.
        let plan = recording_plan(
            json!({}),
            json!({"recording": {"stay_awake": true, "show_touches": true}}),
        );
        assert!(plan.stay_awake && plan.show_touches);
    }

    #[test]
    fn recording_audio_can_be_switched_off() {
        for source in ["none", "off", "NONE"] {
            let plan = recording_plan(json!({"audio_source": source}), json!({}));
            let args = plan.args(Path::new("/tmp/clip.mp4"));
            assert!(args.contains(&"--no-audio".to_string()), "источник: {source}");
            assert!(!args.iter().any(|arg| arg.starts_with("--audio-source")));
        }
    }

    #[test]
    fn recording_rejects_unknown_formats() {
        let error = RecordingPlan::resolve(
            &object(json!({"format": "avi"})),
            &Map::new(),
            PathBuf::from("/x"),
        )
        .expect_err("формат отвергнут");
        assert_eq!(error.status, 400);
        assert_eq!(error.detail, "Unsupported format");

        // Регистр не важен — Python приводит к нижнему.
        assert!(
            RecordingPlan::resolve(
                &object(json!({"format": "MP4"})),
                &Map::new(),
                PathBuf::from("/x")
            )
            .is_ok()
        );
    }

    #[test]
    fn recording_file_name_matches_python_pattern() {
        let plan = recording_plan(json!({"file_prefix": "my clip"}), json!({}));
        assert_eq!(
            plan.file_name("20260812_134153"),
            "my_clip_20260812_134153.mp4"
        );
    }

    #[test]
    fn command_line_joins_with_spaces() {
        assert_eq!(
            command_line(Path::new("scrcpy"), &["--serial".into(), "X".into()]),
            "scrcpy --serial X"
        );
    }

    #[test]
    fn restore_queue_hands_each_ticket_out_once() {
        let mut restore = Restore::default();
        restore
            .entries
            .push(("global", "stay_on_while_plugged_in", Some("0".to_string())));

        let ticket = remember(Path::new("adb"), Some("R58M123"), restore).expect("билет выдан");
        assert!(take(ticket).is_some());
        // Второй раз — уже никому: иначе выход из приложения и фоновая задача
        // восстановили бы настройку дважды.
        assert!(take(ticket).is_none());

        // Пустой карте билет не нужен.
        assert!(remember(Path::new("adb"), None, Restore::default()).is_none());
    }
}
