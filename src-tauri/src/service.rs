//! Сервисные adb-команды — порт `mkdsc/web/service_commands.py`.
//!
//! Три эндпоинта: список предопределённых команд, запуск предопределённой и
//! запуск произвольной. Последний — **единственное место во всём приложении,
//! которое сознательно пускает пользовательский ввод в `adb shell`**, поэтому
//! фильтр опасных команд перенесён буквально, вместе с разбором первого токена
//! и семантикой `shlex`.
//!
//! Переписывать его на «понятные» регулярки нельзя: прежняя версия проверяла
//! подстроки и пропускала `rm -r`, `dd` и `pm uninstall`, одновременно блокируя
//! безобидный `dumpsys | grep format`. Тесты на обход — в конце файла.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};

use crate::error::ApiError;

/// Таймаут одной команды — `DEFAULT_TIMEOUT_SECONDS`.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Сколько ещё ждём вывод убитой по таймауту команды.
///
/// Трубу мог унаследовать поднятый демон adb — тогда чтение до EOF не кончится
/// никогда, хотя сама команда давно мертва. Столько же ждёт `adb pair`.
const READ_AFTER_KILL: Duration = Duration::from_secs(5);

/// Предел длины произвольной команды — как `len(text) > 2000` в Python.
const MAX_COMMAND_LEN: usize = 2000;

/// Предопределённые команды: имя → (что выполнить, сколько строк оставить).
///
/// Порядок важен: он уезжает в `GET /api/service/commands` и задаёт порядок
/// кнопок в сервисном меню.
const PREDEFINED_COMMANDS: [(&str, &[&str], Option<usize>); 13] = [
    ("battery", &["dumpsys battery"], Some(80)),
    ("wifi", &["cmd wifi status"], Some(80)),
    ("top", &["top -n 1 -b"], Some(50)),
    ("props", &["getprop"], Some(100)),
    ("memory", &["cat /proc/meminfo"], Some(80)),
    ("cpu", &["cat /proc/cpuinfo"], Some(50)),
    ("disk", &["df -h"], None),
    ("packages", &["pm list packages"], Some(100)),
    ("screen", &["wm size", "wm density"], None),
    ("processes", &["ps -A"], Some(50)),
    ("uptime", &["uptime"], Some(20)),
    ("network", &["ip addr show"], Some(120)),
    ("thermal", &["dumpsys thermalservice"], Some(120)),
];

/// Описания команд для интерфейса — `COMMAND_DESCRIPTIONS`.
const COMMAND_DESCRIPTIONS: [(&str, &str); 13] = [
    ("battery", "Battery status and health"),
    ("wifi", "WiFi connection info"),
    ("top", "Running processes (top)"),
    ("props", "System properties"),
    ("memory", "Memory info (/proc/meminfo)"),
    ("cpu", "CPU info (/proc/cpuinfo)"),
    ("disk", "Disk usage (df -h)"),
    ("packages", "Installed packages"),
    ("screen", "Screen size and density"),
    ("processes", "Process list (ps)"),
    ("uptime", "Device uptime"),
    ("network", "Network interfaces"),
    ("thermal", "Thermal service status"),
];

/// Символы, из-за которых команда уходит в `sh -c`, — `SHELL_META_CHARS`.
const SHELL_META_CHARS: [char; 11] = ['|', '&', ';', '<', '>', '(', ')', '$', '`', '\\', '\n'];

/// Диагностические утилиты, которые ничего не ломают. Всё остальное требует
/// явного подтверждения от пользователя (`confirm=true`).
const SAFE_BINARIES: [&str; 33] = [
    "cat", "cmd", "date", "df", "dmesg", "du", "dumpsys", "echo", "free", "getprop", "grep",
    "head", "hostname", "id", "ifconfig", "ip", "ls", "lsof", "netstat", "printenv", "ps", "pwd",
    "sed", "sort", "stat", "tail", "top", "uname", "uniq", "uptime", "wc", "whoami", "wm",
];

/// Команды, которые не выполняются никогда — ни с каким подтверждением.
const HARD_BLOCKED_BINARIES: [&str; 6] = [
    "fastboot",
    "flash_image",
    "mkfs",
    "mke2fs",
    "newfs_msdos",
    "recovery",
];

/// Пути, удаление которых равносильно порче устройства.
const PROTECTED_TARGETS: [&str; 12] = [
    "/",
    "/*",
    "/system",
    "/data",
    "/vendor",
    "/sdcard",
    "/storage",
    "/storage/emulated",
    "/storage/emulated/0",
    "/mnt",
    "/proc",
    "/dev",
];

/// Аргументы `reboot`, ведущие в режимы, из которых устройство само не выйдет.
const BLOCKED_REBOOT_TARGETS: [&str; 4] = ["bootloader", "recovery", "fastboot", "edl"];

/// Тело ответа `GET /api/service/commands`.
pub fn command_list() -> Value {
    json!({
        "commands": PREDEFINED_COMMANDS.map(|(name, _, _)| name),
        "descriptions": COMMAND_DESCRIPTIONS
            .iter()
            .map(|(name, text)| ((*name).to_string(), json!(text)))
            .collect::<serde_json::Map<String, Value>>(),
    })
}

/// Что выполняет предопределённая команда, или 404 с текстом обработчика.
///
/// Отдельно от запуска, чтобы неизвестное имя отвечало 404 раньше, чем
/// «инструменты ещё готовятся»: в Python проверка имени тоже стоит до поиска
/// adb.
pub fn lookup(name: &str) -> Result<Predefined, ApiError> {
    PREDEFINED_COMMANDS
        .iter()
        .find(|(command_name, _, _)| *command_name == name)
        .map(|(_, commands, max_lines)| Predefined {
            commands,
            max_lines: *max_lines,
        })
        .ok_or_else(|| ApiError::new(404, unknown_command_detail(name)))
}

/// Найденная предопределённая команда.
pub struct Predefined {
    commands: &'static [&'static str],
    max_lines: Option<usize>,
}

/// Выполняет предопределённую команду.
pub async fn run_predefined(
    adb: &Path,
    command: &Predefined,
    serial: Option<&str>,
) -> Result<Value, ApiError> {
    Ok(run_all(adb, command.commands, serial, command.max_lines)
        .await?
        .into_value())
}

/// Выполняет произвольную команду.
///
/// Фильтр (`validate_custom_command`) вызывается **до** этого, отдельно: в
/// Python он тоже стоит перед поиском adb, чтобы запрещённая команда получала
/// отказ, а не «инструменты ещё готовятся».
pub async fn run_custom(adb: &Path, command: &str, serial: Option<&str>) -> Result<Value, ApiError> {
    // `max_lines=None`: вывод произвольной команды не подрезается — так же
    // вызывает `run_adb_command` питоновский обработчик.
    Ok(run_all(adb, &[command], serial, None).await?.into_value())
}

/// Текст 404 — включая список доступных команд в питоновском виде списка.
fn unknown_command_detail(name: &str) -> String {
    let available = PREDEFINED_COMMANDS
        .iter()
        .map(|(command_name, _, _)| format!("'{command_name}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Unknown command: {name}. Available: [{available}]")
}

// ---------------------------------------------------------------------------
// Фильтр опасных команд
// ---------------------------------------------------------------------------

/// Пропускает диагностические команды, остальное — только с `confirm`.
///
/// Разбор идёт по первому токену каждого звена конвейера, а не по подстрокам:
/// прежний блок-лист не ловил ни `rm -r /sdcard`, ни `dd`, зато блокировал
/// безобидный `dumpsys | grep format`.
///
/// Ошибки: 400 — пусто, слишком длинно, запрещённый бинарь, `reboot` в
/// bootloader/recovery, удаление защищённого пути; 403 — команда выходит за
/// пределы диагностики и не подтверждена.
pub fn validate_custom_command(command: &str, confirm: bool) -> Result<(), ApiError> {
    let text = command.trim();
    if text.is_empty() {
        return Err(ApiError::new(400, "Command required"));
    }
    // Длина считается в символах, как `len()` в Python, а не в байтах: иначе
    // кириллица в комментарии к команде срезала бы предел вдвое.
    if text.chars().count() > MAX_COMMAND_LEN {
        return Err(ApiError::new(400, "Command too long"));
    }

    let segments: Vec<&str> = split_pipeline(text)
        .into_iter()
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return Err(ApiError::new(400, "Command required"));
    }

    // Множество, а не список: причины дедуплицируются и сортируются, как
    // `sorted(set(unsafe_reasons))` в Python.
    let mut unsafe_reasons = std::collections::BTreeSet::new();
    if has_unsafe_constructs(text) {
        unsafe_reasons.insert("redirection or command substitution".to_string());
    }

    for segment in segments {
        // Незакрытая кавычка — «непонятно, что выполняется», то есть небезопасно.
        let Ok(tokens) = shlex_split(segment) else {
            unsafe_reasons.insert("unparsable command".to_string());
            continue;
        };
        let Some(first) = tokens.first() else {
            unsafe_reasons.insert("unparsable command".to_string());
            continue;
        };

        let binary = binary_name(first);

        if HARD_BLOCKED_BINARIES.contains(&binary.as_str()) {
            return Err(ApiError::new(
                400,
                format!("Command '{binary}' is never allowed from the web panel"),
            ));
        }

        if binary == "reboot"
            && tokens[1..]
                .iter()
                .any(|arg| BLOCKED_REBOOT_TARGETS.contains(&arg.to_lowercase().as_str()))
        {
            return Err(ApiError::new(
                400,
                "Rebooting into bootloader/recovery is not allowed from the web panel",
            ));
        }

        if binary == "rm" || binary == "rmdir" {
            for arg in &tokens[1..] {
                if arg.starts_with('-') {
                    continue;
                }
                // Обе проверки нужны: `rm -rf /sdcard/` совпадает после снятия
                // слэшей, а сам `/` после снятия превратился бы в пустую строку.
                if PROTECTED_TARGETS.contains(&arg.trim_end_matches('/'))
                    || PROTECTED_TARGETS.contains(&arg.as_str())
                {
                    return Err(ApiError::new(
                        400,
                        format!("Refusing to delete protected path '{arg}'"),
                    ));
                }
            }
        }

        if !SAFE_BINARIES.contains(&binary.as_str()) {
            unsafe_reasons.insert(format!(
                "'{binary}' is not a read-only diagnostic command"
            ));
        }
    }

    if !unsafe_reasons.is_empty() && !confirm {
        let reasons = unsafe_reasons.into_iter().collect::<Vec<_>>().join("; ");
        return Err(ApiError::new(
            403,
            format!(
                "This command can modify the device ({reasons}). \
                 Re-send with confirm=true to run it anyway."
            ),
        ));
    }

    Ok(())
}

/// Разбивает команду на звенья конвейера — `_PIPELINE_SPLIT_RE`
/// (`\|\||&&|[|;\n]`).
///
/// Написано вручную, а не регуляркой: ради одного разбиения тащить `regex`
/// незачем, а поведение здесь важнее краткости.
fn split_pipeline(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut segments = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        // `||` и `&&` — двухсимвольные разделители; одиночный `&` разделителем
        // не считается, как и в питоновской регулярке.
        let width = match (bytes[index], bytes.get(index + 1)) {
            (b'|', Some(b'|')) | (b'&', Some(b'&')) => 2,
            (b'|', _) | (b';', _) | (b'\n', _) => 1,
            _ => 0,
        };

        if width == 0 {
            index += 1;
            continue;
        }

        segments.push(&text[start..index]);
        index += width;
        start = index;
    }

    segments.push(&text[start..]);
    segments
}

/// Перенаправления и подстановка команд — `_UNSAFE_CONSTRUCTS_RE`
/// (``[<>`]|\$\(``).
fn has_unsafe_constructs(text: &str) -> bool {
    text.contains(['<', '>', '`']) || text.contains("$(")
}

/// Имя бинаря из токена: `/system/bin/rm` → `rm`.
fn binary_name(token: &str) -> String {
    token.rsplit('/').next().unwrap_or(token).to_lowercase()
}

/// Ошибка разбора — те же два случая, на которых спотыкается `shlex.split`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShlexError {
    /// Кавычка открылась и не закрылась.
    NoClosingQuote,
    /// Обратный слэш в самом конце строки.
    NoEscapedCharacter,
}

impl ShlexError {
    /// Текст ошибки — дословно тот, что печатает `shlex`.
    fn detail(self) -> &'static str {
        match self {
            Self::NoClosingQuote => "No closing quotation",
            Self::NoEscapedCharacter => "No escaped character",
        }
    }
}

/// Разбирает строку как `shlex.split(text)` (POSIX-режим, без комментариев).
///
/// Нужен именно он, а не разбиение по пробелам: фильтр смотрит на первый токен,
/// и `"rm" -rf /` с пробелом внутри кавычек обязан разбираться так же, как в
/// Python, иначе проверка увидит не тот бинарь.
pub fn shlex_split(text: &str) -> Result<Vec<String>, ShlexError> {
    /// Пробельные символы `shlex` — только эти четыре, юникодных пробелов там нет.
    fn is_space(ch: char) -> bool {
        matches!(ch, ' ' | '\t' | '\r' | '\n')
    }

    enum State {
        Plain,
        Single,
        Double,
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    // Отдельный флаг: `''` даёт пустой токен, а не «токена не было».
    let mut started = false;
    let mut state = State::Plain;
    let mut chars = text.chars();

    while let Some(ch) = chars.next() {
        match state {
            State::Plain => match ch {
                '\\' => {
                    let next = chars.next().ok_or(ShlexError::NoEscapedCharacter)?;
                    current.push(next);
                    started = true;
                }
                '\'' => {
                    state = State::Single;
                    started = true;
                }
                '"' => {
                    state = State::Double;
                    started = true;
                }
                _ if is_space(ch) => {
                    if started {
                        tokens.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                _ => {
                    current.push(ch);
                    started = true;
                }
            },
            State::Single => {
                // Внутри одинарных кавычек нет ни экранирования, ни подстановок.
                if ch == '\'' {
                    state = State::Plain;
                } else {
                    current.push(ch);
                }
            }
            State::Double => match ch {
                '"' => state = State::Plain,
                '\\' => {
                    // В двойных кавычках слэш экранирует только `"` и себя;
                    // перед остальным он остаётся обычным символом.
                    let next = chars.next().ok_or(ShlexError::NoEscapedCharacter)?;
                    if next != '"' && next != '\\' {
                        current.push('\\');
                    }
                    current.push(next);
                }
                _ => current.push(ch),
            },
        }
    }

    if !matches!(state, State::Plain) {
        return Err(ShlexError::NoClosingQuote);
    }
    if started {
        tokens.push(current);
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Запуск
// ---------------------------------------------------------------------------

/// Ответ одной или нескольких команд — `CommandResponse`.
#[derive(Debug, Clone)]
pub struct CommandResponse {
    success: bool,
    output: String,
    error: Option<String>,
    command: String,
}

impl CommandResponse {
    fn into_value(self) -> Value {
        json!({
            "success": self.success,
            "output": self.output,
            "error": self.error,
            "command": self.command,
        })
    }
}

/// Выполняет одну или несколько команд подряд и склеивает результаты.
///
/// Склейка — из `run_adb_command`: выводы через пустую строку, ошибки через
/// перевод строки, командная строка через ` ; `. `success` — только если
/// отработали все.
async fn run_all(
    adb: &Path,
    commands: &[&str],
    serial: Option<&str>,
    max_lines: Option<usize>,
) -> Result<CommandResponse, ApiError> {
    let mut outputs = Vec::new();
    let mut errors = Vec::new();
    let mut executed = Vec::new();
    let mut success = true;

    for command in commands {
        let result = run_single(adb, command, serial, max_lines).await?;
        executed.push(result.command);
        if !result.output.is_empty() {
            outputs.push(result.output);
        }
        if !result.success {
            success = false;
            // Пустой текст ошибки не копится: `if result.error` в Python пустую
            // строку не пропускает, и в ответе не должно появиться `error: "\n"`.
            if let Some(error) = result.error.filter(|error| !error.is_empty()) {
                errors.push(error);
            }
        }
    }

    Ok(CommandResponse {
        success,
        output: outputs.join("\n\n"),
        error: (!errors.is_empty()).then(|| errors.join("\n")),
        command: executed.join(" ; "),
    })
}

async fn run_single(
    adb: &Path,
    command: &str,
    serial: Option<&str>,
    max_lines: Option<usize>,
) -> Result<CommandResponse, ApiError> {
    let args = build_adb_args(serial, command)?;
    let line = std::iter::once(adb.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");

    let capture = capture(adb, &args).await?;
    // Пустой stdout — показываем stderr: команда могла всё сказать туда.
    let raw = if capture.stdout.is_empty() {
        capture.stderr.clone()
    } else {
        capture.stdout.clone()
    };
    let mut output = trim_output(&raw, max_lines);

    if capture.timed_out {
        let seconds = DEFAULT_TIMEOUT.as_secs();
        if !output.is_empty() {
            output.push_str(&format!("\n\n[Timed out after {seconds} seconds]"));
        }
        return Ok(CommandResponse {
            success: false,
            output,
            error: Some(format!("Command timed out after {seconds} seconds")),
            command: line,
        });
    }

    Ok(CommandResponse {
        success: capture.code == 0,
        output,
        error: (capture.code != 0).then_some(capture.stderr),
        command: line,
    })
}

/// Собирает аргументы adb — `_build_adb_command`.
///
/// Команда с метасимволами уходит целиком в `sh -c`, остальное разбирается
/// `shlex` и передаётся аргументами. Незакрытая кавычка здесь — 500: в Python
/// `shlex.split` кидает `ValueError` мимо `try`, и обработчик отвечает ровно
/// этим текстом. Дотуда доезжает только команда с `confirm=true` — фильтр без
/// подтверждения такую уже отверг.
fn build_adb_args<'a>(serial: Option<&'a str>, command: &'a str) -> Result<Vec<String>, ApiError> {
    let mut args: Vec<String> = Vec::new();
    if let Some(serial) = serial {
        args.push("-s".to_string());
        args.push(serial.to_string());
    }
    args.push("shell".to_string());

    if command.contains(SHELL_META_CHARS) {
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());
    } else {
        let tokens = shlex_split(command).map_err(|err| ApiError::internal(err.detail()))?;
        args.extend(tokens);
    }

    Ok(args)
}

/// Обрезает вывод до `max_lines` строк — `_trim_output`.
fn trim_output(output: &str, max_lines: Option<usize>) -> String {
    // `None` и `0` одинаково означают «не обрезать»: в Python оба ложны.
    let Some(max_lines) = max_lines.filter(|max| *max > 0) else {
        return output.to_string();
    };
    if output.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return output.to_string();
    }

    let trimmed = lines[..max_lines].join("\n");
    let rest = lines.len() - max_lines;
    format!("{trimmed}\n... ({rest} more lines truncated)")
}

/// Результат запуска с сохранением уже полученного вывода.
struct Capture {
    code: i32,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

/// Запускает adb, вычитывая трубы отдельными задачами.
///
/// Не `devices::run_adb`: тот на таймауте отдаёт пустой вывод, а `subprocess`
/// в Python возвращает через `TimeoutExpired` всё, что процесс успел напечатать.
/// Для диагностики это и есть самое ценное — команда, зависшая на середине
/// `dumpsys`, всё равно должна показать первую половину.
async fn capture(adb: &Path, args: &[String]) -> Result<Capture, ApiError> {
    let mut command = tokio::process::Command::new(adb);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::set_no_window(command.as_std_mut());

    let mut child = command
        .spawn()
        .map_err(|err| ApiError::internal(format!("failed to run adb: {err}")))?;

    let stdout = child.stdout.take().map(crate::devices::read_to_string);
    let stderr = child.stderr.take().map(crate::devices::read_to_string);

    let status = tokio::time::timeout(DEFAULT_TIMEOUT, child.wait()).await;
    let timed_out = status.is_err();
    if timed_out {
        let _ = child.kill().await;
    }

    let mut collected = Vec::new();
    for task in [stdout, stderr].into_iter().flatten() {
        let text = tokio::time::timeout(READ_AFTER_KILL, task)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        collected.push(text);
    }
    let mut collected = collected.into_iter();

    Ok(Capture {
        code: match status {
            Ok(Ok(status)) => status.code().unwrap_or(-1),
            // Убитый по таймауту процесс и сбой ожидания одинаково «не ноль».
            _ => -1,
        },
        stdout: collected.next().unwrap_or_default(),
        stderr: collected.next().unwrap_or_default(),
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(command: &str, confirm: bool) -> Option<u16> {
        validate_custom_command(command, confirm)
            .err()
            .map(|error| error.status)
    }

    fn detail_of(command: &str, confirm: bool) -> String {
        validate_custom_command(command, confirm)
            .err()
            .map(|error| error.detail)
            .unwrap_or_default()
    }

    /// Диагностика проходит без подтверждения — в том числе с конвейером.
    ///
    /// `dumpsys | grep format` — тот самый случай, который блокировал прежний
    /// фильтр по подстрокам: слово «format» в аргументе он считал форматированием.
    #[test]
    fn diagnostics_pass_without_confirmation() {
        for command in [
            "dumpsys battery",
            "dumpsys | grep format",
            "cat /proc/meminfo | grep MemTotal",
            "ps -A | grep com.android",
            "getprop ro.build.version.release",
            "ls -la /sdcard",
            "top -n 1 -b && uptime",
            "df -h; free",
            "echo 'hello world'",
            "grep -r dd /proc/version",
        ] {
            assert_eq!(
                validate_custom_command(command, false).map_err(|e| e.detail),
                Ok(()),
                "команда должна проходить: {command}"
            );
        }
    }

    /// Изменяющие устройство команды требуют `confirm=true`.
    ///
    /// Ровно то, что пропускал прежний блок-лист подстрок.
    #[test]
    fn modifying_commands_need_confirmation() {
        for command in [
            "rm -r /sdcard/DCIM",
            "dd if=/dev/zero of=/sdcard/x",
            "pm uninstall com.example",
            "settings put system x 1",
            "svc power reboot",
            "am force-stop com.example",
            "chmod 777 /sdcard/x",
            "reboot",
        ] {
            assert_eq!(status_of(command, false), Some(403), "без confirm: {command}");
            assert_eq!(status_of(command, true), None, "с confirm: {command}");
        }

        // Текст сверен с питоновским обработчиком дословно: его показывает
        // диалог подтверждения, и «примерно такой же» тут не годится.
        assert_eq!(
            detail_of("pm uninstall com.example", false),
            "This command can modify the device ('pm' is not a read-only diagnostic command). \
             Re-send with confirm=true to run it anyway."
        );
    }

    /// Абсолютный путь не прячет бинарь: сравнивается имя файла.
    #[test]
    fn absolute_paths_do_not_hide_the_binary() {
        assert_eq!(status_of("/system/bin/fastboot flash", true), Some(400));
        assert_eq!(status_of("/system/bin/rm -rf /sdcard", true), Some(400));
        // И регистр тоже: `binary_name` приводит к нижнему.
        assert_eq!(status_of("FASTBOOT devices", true), Some(400));
    }

    /// Запрещённые бинари не выполняются даже с подтверждением.
    #[test]
    fn hard_blocked_binaries_ignore_confirmation() {
        for command in [
            "fastboot devices",
            "mkfs /dev/block/sda",
            "mke2fs /dev/block/sda",
            "newfs_msdos /dev/block/sda",
            "flash_image boot boot.img",
            "recovery --wipe_data",
        ] {
            assert_eq!(status_of(command, true), Some(400), "заблокировано: {command}");
            assert!(detail_of(command, true).contains("is never allowed"));
        }
        assert_eq!(
            detail_of("/system/bin/fastboot flash", true),
            "Command 'fastboot' is never allowed from the web panel"
        );
        assert_eq!(
            detail_of("rm -rf /sdcard/", true),
            "Refusing to delete protected path '/sdcard/'"
        );

        // В любом звене конвейера, а не только в первом.
        assert_eq!(status_of("dumpsys battery | fastboot devices", true), Some(400));
        assert_eq!(status_of("echo x && mkfs /dev/block/sda", true), Some(400));
        assert_eq!(status_of("echo x; recovery", true), Some(400));
    }

    /// Перезагрузка в bootloader/recovery запрещена: оттуда устройство само не
    /// вернётся, а пользователь у панели физически до него не дотянется.
    #[test]
    fn reboot_into_service_modes_is_blocked() {
        for command in [
            "reboot bootloader",
            "reboot recovery",
            "reboot RECOVERY",
            "reboot fastboot",
            "reboot edl",
            "reboot -p bootloader",
        ] {
            assert_eq!(status_of(command, true), Some(400), "заблокировано: {command}");
        }

        // Обычная перезагрузка — не диагностика, но и не запрет: с confirm можно.
        assert_eq!(status_of("reboot", true), None);
        assert_eq!(status_of("reboot -p", true), None);
    }

    /// Удаление системных путей не проходит ни с каким подтверждением.
    #[test]
    fn protected_paths_are_never_deleted() {
        for command in [
            "rm -rf /",
            "rm -rf /*",
            "rm -rf /system",
            "rm -rf /data",
            "rm -rf /sdcard",
            "rm -rf /sdcard/",
            "rm -rf /storage/emulated/0",
            "rmdir /mnt",
            "rm -rf /vendor",
            "/system/bin/rm -rf /data",
        ] {
            assert_eq!(status_of(command, true), Some(400), "заблокировано: {command}");
            assert!(detail_of(command, true).contains("Refusing to delete protected path"));
        }

        // Подкаталог удалять можно — с подтверждением.
        assert_eq!(status_of("rm -rf /sdcard/DCIM", true), None);
        assert_eq!(status_of("rm -rf /data/local/tmp/x", true), None);
    }

    /// Пустое, слишком длинное и состоящее из одних разделителей — 400.
    #[test]
    fn empty_and_oversized_commands_are_rejected() {
        assert_eq!(detail_of("", false), "Command required");
        assert_eq!(detail_of("   ", false), "Command required");
        assert_eq!(detail_of(" ; | \n ", false), "Command required");
        assert_eq!(detail_of(&"a".repeat(MAX_COMMAND_LEN + 1), true), "Command too long");
        // Ровно предел — ещё можно.
        assert_ne!(detail_of(&"a".repeat(MAX_COMMAND_LEN), true), "Command too long");
    }

    /// Перенаправление и подстановка команд — повод спросить подтверждение.
    #[test]
    fn redirection_and_substitution_need_confirmation() {
        for command in [
            "cat /proc/meminfo > /sdcard/mem.txt",
            "cat < /sdcard/x",
            "echo `id`",
            "echo $(id)",
        ] {
            assert_eq!(status_of(command, false), Some(403), "без confirm: {command}");
            assert!(detail_of(command, false).contains("redirection or command substitution"));
        }
    }

    /// Незакрытая кавычка — «непонятно что», то есть небезопасно.
    #[test]
    fn unparsable_segments_are_unsafe() {
        assert_eq!(status_of("ls \"abc", false), Some(403));
        assert!(detail_of("ls \"abc", false).contains("unparsable command"));
        // С подтверждением фильтр пропускает, а сборка аргументов отвечает 500 —
        // ровно как ValueError из shlex мимо try в Python.
        assert_eq!(status_of("ls \"abc", true), None);
        assert_eq!(
            build_adb_args(None, "ls \"abc").unwrap_err().detail,
            "No closing quotation"
        );
    }

    /// Причины в тексте 403 дедуплицированы и отсортированы.
    #[test]
    fn reasons_are_deduplicated_and_sorted() {
        let detail = detail_of("dd if=x | dd if=y > /sdcard/z", false);
        assert_eq!(
            detail,
            "This command can modify the device ('dd' is not a read-only diagnostic command; \
             redirection or command substitution). Re-send with confirm=true to run it anyway."
        );
    }

    #[test]
    fn pipeline_split_matches_the_python_regex() {
        assert_eq!(split_pipeline("a | b"), ["a ", " b"]);
        assert_eq!(split_pipeline("a || b"), ["a ", " b"]);
        assert_eq!(split_pipeline("a && b"), ["a ", " b"]);
        assert_eq!(split_pipeline("a ; b"), ["a ", " b"]);
        assert_eq!(split_pipeline("a\nb"), ["a", "b"]);
        // Одиночный `&` разделителем не считается — как и в питоновской регулярке.
        assert_eq!(split_pipeline("a & b"), ["a & b"]);
        assert_eq!(split_pipeline("a"), ["a"]);
        assert_eq!(split_pipeline(""), [""]);
    }

    #[test]
    fn shlex_matches_python() {
        let cases: &[(&str, &[&str])] = &[
            ("ls -la", &["ls", "-la"]),
            ("echo 'hello world'", &["echo", "hello world"]),
            ("echo \"hello world\"", &["echo", "hello world"]),
            ("echo a'b'c", &["echo", "abc"]),
            ("echo ''", &["echo", ""]),
            ("echo a\\ b", &["echo", "a b"]),
            ("  spaced   out  ", &["spaced", "out"]),
            ("echo \"a\\\"b\"", &["echo", "a\"b"]),
            // В двойных кавычках слэш перед прочим остаётся символом.
            ("echo \"a\\nb\"", &["echo", "a\\nb"]),
            ("", &[]),
            ("\t\r\n", &[]),
        ];

        for (input, expected) in cases {
            assert_eq!(
                shlex_split(input).expect("разбирается"),
                expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "разбор: {input:?}"
            );
        }

        assert_eq!(shlex_split("ls 'abc"), Err(ShlexError::NoClosingQuote));
        assert_eq!(shlex_split("ls \"abc"), Err(ShlexError::NoClosingQuote));
        assert_eq!(shlex_split("ls abc\\"), Err(ShlexError::NoEscapedCharacter));
    }

    /// Метасимволы уводят команду в `sh -c` целиком, остальное — аргументами.
    #[test]
    fn shell_is_used_only_for_metacharacters() {
        assert_eq!(
            build_adb_args(None, "dumpsys battery").unwrap(),
            ["shell", "dumpsys", "battery"]
        );
        assert_eq!(
            build_adb_args(Some("R58M123"), "dumpsys battery").unwrap(),
            ["-s", "R58M123", "shell", "dumpsys", "battery"]
        );
        assert_eq!(
            build_adb_args(None, "dumpsys | grep level").unwrap(),
            ["shell", "sh", "-c", "dumpsys | grep level"]
        );
        // Кавычки метасимволами не считаются — команда разбирается shlex.
        assert_eq!(
            build_adb_args(None, "echo 'a b'").unwrap(),
            ["shell", "echo", "a b"]
        );
    }

    #[test]
    fn output_is_trimmed_like_python() {
        let text = (1..=10).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");

        assert_eq!(trim_output(&text, None), text);
        assert_eq!(trim_output(&text, Some(10)), text);
        assert_eq!(trim_output(&text, Some(20)), text);
        assert_eq!(
            trim_output(&text, Some(3)),
            "1\n2\n3\n... (7 more lines truncated)"
        );
        assert_eq!(trim_output("", Some(3)), "");
        // CRLF: строки считаются так же, как `splitlines()` в Python.
        assert_eq!(
            trim_output("a\r\nb\r\nc", Some(2)),
            "a\nb\n... (1 more lines truncated)"
        );
    }

    #[test]
    fn command_list_mirrors_python() {
        let list = command_list();
        let commands = list["commands"].as_array().expect("список");
        assert_eq!(commands.len(), PREDEFINED_COMMANDS.len());
        assert_eq!(commands[0], json!("battery"));
        assert_eq!(list["descriptions"]["battery"], json!("Battery status and health"));
        // У каждой команды есть описание — сервисное меню подписывает кнопки.
        for command in commands {
            let name = command.as_str().expect("строка");
            assert!(
                list["descriptions"].get(name).is_some(),
                "нет описания для {name}"
            );
        }
    }

    #[test]
    fn unknown_command_detail_lists_the_available_ones() {
        let detail = unknown_command_detail("bogus");
        assert!(detail.starts_with("Unknown command: bogus. Available: ['battery', 'wifi'"));
        assert!(detail.ends_with("'thermal']"));
    }
}
