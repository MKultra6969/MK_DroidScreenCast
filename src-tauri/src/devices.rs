//! Список подключённых устройств: запуск `adb devices` и разбор вывода.
//!
//! Питоновский оригинал — `mkdsc/tools.py::get_connected_devices`.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;

use crate::error::ApiError;

/// Сколько ждём `adb devices`.
///
/// Первый вызов поднимает демона, поэтому запас заметно больше, чем нужно
/// установившемуся демону. Висеть дольше смысла нет: `App.tsx` опрашивает
/// список раз в 5 секунд и всё равно повторит попытку.
const ADB_TIMEOUT: Duration = Duration::from_secs(15);

/// Устройство из вывода `adb devices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectedDevice {
    pub serial: String,
    /// Строка, а не enum: контракт зеркалит REST, где это просто поле adb
    /// (`device`, `offline`, `unauthorized`, `authorizing`, ...), и список
    /// значений задаёт adb, а не мы.
    pub status: String,
}

/// Разбирает stdout `adb devices`.
///
/// Пустой вывод — пустой список, а не ошибка.
pub fn parse_adb_devices(stdout: &str) -> Vec<ConnectedDevice> {
    stdout
        .trim()
        .split('\n')
        // Первая строка — заголовок `List of devices attached`.
        .skip(1)
        .filter_map(parse_device_line)
        .collect()
}

fn parse_device_line(line: &str) -> Option<ConnectedDevice> {
    // В Python переводы строк нормализовал `text=True`; в Rust его нет, и на
    // Windows '\r' доехал бы до UI как status == "device\r" — сравнения вида
    // `status === 'device'` молча перестали бы срабатывать.
    let line = line.trim_end_matches('\r');

    // Строки без табуляции — шум демона (`* daemon not running; starting now
    // at tcp:5037`, `* daemon started successfully`) и пустые строки.
    // Разделяем ровно один раз: в поле состояния теоретически может оказаться
    // ещё одна табуляция, и разбор по всем упал бы (в Python — с ValueError).
    let (serial, status) = line.split_once('\t')?;

    let serial = serial.trim();
    let status = status.trim();
    if serial.is_empty() || status.is_empty() {
        return None;
    }

    Some(ConnectedDevice {
        serial: serial.to_string(),
        status: status.to_string(),
    })
}

/// Запускает `adb devices` и возвращает разобранный список.
///
/// Ошибки — 500 с текстом от adb: их отдаёт и HTTP-эндпоинт.
pub async fn list_connected(adb: &Path) -> Result<Vec<ConnectedDevice>, ApiError> {
    // `tokio::process`, а не `std::process`: команда вызывается из async-кода,
    // и блокирующее ожидание встало бы прямо в потоке рантайма.
    let mut command = tokio::process::Command::new(adb);
    command
        .arg("devices")
        .stdin(Stdio::null())
        // Опрос идёт раз в 5 секунд; без этого зависший adb копил бы
        // осиротевшие процессы после каждого таймаута.
        .kill_on_drop(true);
    // На Windows иначе на каждый опрос мигало бы окно консоли.
    crate::set_no_window(command.as_std_mut());

    let output = tokio::time::timeout(ADB_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            ApiError::internal(format!(
                "adb devices timed out after {}s",
                ADB_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|err| ApiError::internal(format!("failed to run adb: {err}")))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ApiError::internal(if detail.is_empty() {
            "adb devices failed".to_string()
        } else {
            detail
        }));
    }

    // Серийник USB-устройства приходит от прошивки и не обязан быть валидным
    // UTF-8 — декодируем lossy, чтобы мусорный серийник не ронял весь список.
    Ok(parse_adb_devices(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Случай таблицы: описание, stdout `adb devices`, ожидаемые пары
    /// «серийник — состояние».
    type ParseCase = (
        &'static str,
        &'static str,
        &'static [(&'static str, &'static str)],
    );

    fn parsed(stdout: &str) -> Vec<(String, String)> {
        parse_adb_devices(stdout)
            .into_iter()
            .map(|device| (device.serial, device.status))
            .collect()
    }

    #[test]
    fn parses_adb_devices_output() {
        let cases: &[ParseCase] = &[
            (
                "обычный вывод",
                "List of devices attached\nemulator-5554\tdevice\n",
                &[("emulator-5554", "device")],
            ),
            (
                "windows CRLF",
                "List of devices attached\r\nR58M123ABCD\tdevice\r\n",
                &[("R58M123ABCD", "device")],
            ),
            (
                "CRLF без завершающего перевода строки",
                "List of devices attached\r\nR58M123ABCD\tdevice\r",
                &[("R58M123ABCD", "device")],
            ),
            (
                "CRLF и несколько устройств",
                "List of devices attached\r\nR58M123ABCD\tdevice\r\nemulator-5554\toffline\r\n\r\n",
                &[("R58M123ABCD", "device"), ("emulator-5554", "offline")],
            ),
            (
                "шум демона",
                "* daemon not running; starting now at tcp:5037\n* daemon started successfully\n\
                 List of devices attached\nemulator-5554\tdevice\n",
                &[("emulator-5554", "device")],
            ),
            (
                "шум демона с CRLF",
                "* daemon not running; starting now at tcp:5037\r\n* daemon started successfully\r\n\
                 List of devices attached\r\nemulator-5554\tdevice\r\n",
                &[("emulator-5554", "device")],
            ),
            (
                "unauthorized",
                "List of devices attached\nR58M123ABCD\tunauthorized\n",
                &[("R58M123ABCD", "unauthorized")],
            ),
            (
                "offline",
                "List of devices attached\nR58M123ABCD\toffline\n",
                &[("R58M123ABCD", "offline")],
            ),
            (
                "устройство по Wi-Fi",
                "List of devices attached\n192.168.1.5:5555\tdevice\n",
                &[("192.168.1.5:5555", "device")],
            ),
            (
                "смешанный список",
                "List of devices attached\nemulator-5554\tdevice\n192.168.1.5:5555\tdevice\n\
                 R58M123ABCD\tunauthorized\n",
                &[
                    ("emulator-5554", "device"),
                    ("192.168.1.5:5555", "device"),
                    ("R58M123ABCD", "unauthorized"),
                ],
            ),
            ("пустой вывод", "", &[]),
            ("только пробелы", "   \r\n\r\n", &[]),
            ("только заголовок", "List of devices attached\n", &[]),
            ("только заголовок, CRLF", "List of devices attached\r\n\r\n", &[]),
            (
                "устройств нет, но демон шумел",
                "* daemon not running; starting now at tcp:5037\nList of devices attached\n",
                &[],
            ),
            (
                // adb такого не печатает, но разбор не должен ни падать, ни
                // терять строку — ровно ради этого splitn(2), а не split.
                "лишняя табуляция",
                "List of devices attached\nemulator-5554\tdevice\textra\n",
                &[("emulator-5554", "device\textra")],
            ),
        ];

        for (name, stdout, expected) in cases {
            let expected: Vec<(String, String)> = expected
                .iter()
                .map(|(serial, status)| (serial.to_string(), status.to_string()))
                .collect();
            assert_eq!(parsed(stdout), expected, "случай: {name}");
        }
    }

    #[test]
    fn status_never_keeps_carriage_return() {
        // Отдельно от таблицы: именно эта ошибка ломается тихо — UI показал бы
        // «device» с переносом, а `status === 'device'` перестал бы работать.
        for device in parse_adb_devices("List of devices attached\r\nABC\tdevice\r\n") {
            assert!(!device.status.contains('\r'), "status: {:?}", device.status);
            assert!(!device.serial.contains('\r'), "serial: {:?}", device.serial);
        }
    }
}
