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
