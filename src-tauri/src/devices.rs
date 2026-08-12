//! Операции adb над устройствами: список, подключение, спаривание, tcpip.
//!
//! Питоновские оригиналы — `mkdsc/tools.py` (`get_connected_devices`,
//! `restart_adb_server`, `get_device_wifi_ip`) и `mkdsc/web/server.py`
//! (`_run_pair` и обработчики пяти эндпоинтов подключения).
//!
//! Общее правило: **каждый запуск adb с таймаутом**. `connect` к недоступному
//! адресу висит десятками секунд, а `pair` без таймаута не возвращался никогда.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::ApiError;

/// Сколько ждём `adb devices`.
///
/// Первый вызов поднимает демона, поэтому запас заметно больше, чем нужно
/// установившемуся демону. Висеть дольше смысла нет: `App.tsx` всё равно
/// перезапросит список (при живом потоке событий — с его пуша, иначе —
/// поллингом раз в 5 секунд).
const ADB_TIMEOUT: Duration = Duration::from_secs(15);

/// `connect`, `disconnect`, `tcpip` — как `timeout=15` в `mkdsc/web/server.py`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// `kill-server` и `start-server` — как `timeout=30` в `restart_adb_server`.
const SERVER_TIMEOUT: Duration = Duration::from_secs(30);

/// `adb shell` — как `DEFAULT_CMD_TIMEOUT` в `mkdsc/tools.py`.
const SHELL_TIMEOUT: Duration = Duration::from_secs(20);

/// `adb pair` — как `timeout=30` в `_run_pair`.
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

/// Сколько ещё ждём вывод убитого `adb pair` — как `communicate(timeout=5)`.
const PAIR_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Код возврата на таймауте — `TIMEOUT_RETURNCODE` из `mkdsc/tools.py`.
const TIMEOUT_RETURNCODE: i32 = 124;

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

/// Результат запуска adb — аналог `CompletedProcess` из `run_cmd`.
#[derive(Debug, Clone)]
pub struct AdbOutput {
    /// Код возврата. Процесс, убитый сигналом, кода не имеет — тогда `-1`;
    /// в любом случае это «не ноль», то есть неуспех.
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Процесс не уложился в таймаут и был убит.
    ///
    /// Отдельным полем, а не только кодом 124: код теоретически может прийти
    /// и от самого adb, а перепутать одно с другим — значит показать
    /// пользователю «истекло время» там, где команда честно отработала.
    pub timed_out: bool,
}

impl AdbOutput {
    pub fn success(&self) -> bool {
        self.code == 0
    }

    /// `result.stdout + result.stderr` — ровно то, что уезжает в поле `output`
    /// ответов `/api/connect`, `/api/tcpip` и попадает пользователю на экран.
    pub fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Запускает adb с таймаутом.
///
/// Таймаут — не ошибка, а результат с кодом 124 и текстом в `stderr`: так его
/// собирает `run_cmd` в `mkdsc/tools.py`, и так его видит пользователь в поле
/// `output`. Ошибкой (500) остаётся только невозможность запустить процесс.
pub async fn run_adb(adb: &Path, args: &[&str], timeout: Duration) -> Result<AdbOutput, ApiError> {
    // `tokio::process`, а не `std::process`: команда вызывается из async-кода,
    // и блокирующее ожидание встало бы прямо в потоке рантайма.
    let mut command = tokio::process::Command::new(adb);
    command
        .args(args)
        .stdin(Stdio::null())
        // Без этого каждый таймаут оставлял бы после себя осиротевший процесс
        // adb — tokio сам их не убивает.
        .kill_on_drop(true);
    // На Windows иначе на каждый вызов мигало бы окно консоли.
    crate::set_no_window(command.as_std_mut());

    let Ok(result) = tokio::time::timeout(timeout, command.output()).await else {
        return Ok(AdbOutput {
            code: TIMEOUT_RETURNCODE,
            stdout: String::new(),
            stderr: format!("Command timed out after {} seconds", timeout.as_secs()),
            timed_out: true,
        });
    };

    let output = result.map_err(|err| ApiError::internal(format!("failed to run adb: {err}")))?;

    Ok(AdbOutput {
        code: output.status.code().unwrap_or(-1),
        // Вывод adb не обязан быть валидным UTF-8: серийник приходит от
        // прошивки, а имя устройства — от пользователя. Декодируем lossy,
        // чтобы мусорный байт не ронял всю команду.
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out: false,
    })
}

/// Аргументы с необязательным `-s <serial>` — как `_adb_cmd` в Python.
///
/// Без `-s` adb при двух подключённых устройствах отвечает «more than one
/// device», и команда молча не срабатывала.
fn with_serial<'a>(serial: Option<&'a str>, args: &[&'a str]) -> Vec<&'a str> {
    let mut result = Vec::with_capacity(args.len() + 2);
    if let Some(serial) = serial {
        result.push("-s");
        result.push(serial);
    }
    result.extend_from_slice(args);
    result
}

/// Запускает `adb devices` и возвращает разобранный список.
///
/// Ошибки — 500 с текстом от adb: их отдаёт и HTTP-эндпоинт.
pub async fn list_connected(adb: &Path) -> Result<Vec<ConnectedDevice>, ApiError> {
    let output = run_adb(adb, &["devices"], ADB_TIMEOUT).await?;

    if output.timed_out {
        return Err(ApiError::internal(format!(
            "adb devices timed out after {}s",
            ADB_TIMEOUT.as_secs()
        )));
    }

    if !output.success() {
        let detail = output.stderr.trim();
        return Err(ApiError::internal(if detail.is_empty() {
            "adb devices failed".to_string()
        } else {
            detail.to_string()
        }));
    }

    Ok(parse_adb_devices(&output.stdout))
}

/// `adb connect <address>` — `POST /api/connect`.
pub async fn connect(adb: &Path, address: &str) -> Result<AdbOutput, ApiError> {
    run_adb(adb, &["connect", address], CONNECT_TIMEOUT).await
}

/// `adb disconnect <address>` — `POST /api/disconnect`.
pub async fn disconnect(adb: &Path, address: &str) -> Result<AdbOutput, ApiError> {
    run_adb(adb, &["disconnect", address], CONNECT_TIMEOUT).await
}

/// `adb tcpip <port>` — переводит устройство в режим подключения по сети.
pub async fn tcpip(adb: &Path, serial: Option<&str>, port: &str) -> Result<AdbOutput, ApiError> {
    run_adb(
        adb,
        &with_serial(serial, &["tcpip", port]),
        CONNECT_TIMEOUT,
    )
    .await
}

/// Перезапуск adb-сервера — кнопка в диагностике.
///
/// Коды возврата игнорируются, как и в `restart_adb_server`: `kill-server` на
/// незапущенном сервере отвечает ненулём, и падать на этом незачем.
pub async fn restart_server(adb: &Path) -> Result<(), ApiError> {
    run_adb(adb, &["kill-server"], SERVER_TIMEOUT).await?;
    run_adb(adb, &["start-server"], SERVER_TIMEOUT).await?;
    Ok(())
}

/// IPv4-адрес устройства в Wi-Fi — `get_device_wifi_ip`.
///
/// `None`, если команда не отработала или адреса в выводе нет: эндпоинт
/// `/api/tcpip` отдаёт его как `null`, а UI просто не подставляет адрес.
pub async fn wifi_ip(adb: &Path, serial: Option<&str>) -> Result<Option<String>, ApiError> {
    let output = run_adb(
        adb,
        &with_serial(serial, &["shell", "ip", "addr", "show", "wlan0"]),
        SHELL_TIMEOUT,
    )
    .await?;

    if !output.success() {
        return Ok(None);
    }

    Ok(parse_wifi_ip(&output.stdout))
}

/// Достаёт IPv4 из вывода `ip addr show wlan0`.
///
/// Ищется первая строка с `inet ` без `inet6`; адрес в ней идёт вторым полем
/// и записан с маской (`192.168.1.5/24`).
pub fn parse_wifi_ip(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .filter(|line| line.contains("inet ") && !line.contains("inet6"))
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            parts.next()?;
            let address = parts.next()?;
            Some(address.split('/').next().unwrap_or(address).to_string())
        })
}

/// Запускает `adb pair <address>` и передаёт код спаривания в stdin.
///
/// `adb pair` спрашивает код интерактивно, поэтому здесь не `output()`, а
/// ручной stdin — как `stdin=PIPE` в `_run_pair`. Возвращает вывод процесса;
/// успех вызывающий определяет по подстроке в нём.
///
/// Таймаут обязателен: без него недоступный адрес вешал вызов навсегда —
/// именно этот баг чинили в Python. По таймауту процесс убивается явно, иначе
/// он остался бы висеть до выхода из приложения.
pub async fn pair(adb: &Path, address: &str, code: &str) -> Result<String, ApiError> {
    let mut command = tokio::process::Command::new(adb);
    command
        .arg("pair")
        .arg(address)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::set_no_window(command.as_std_mut());

    let mut child = command
        .spawn()
        .map_err(|err| ApiError::internal(format!("failed to run adb: {err}")))?;

    // Трубы читаются отдельными задачами по двум причинам: процесс, заполнивший
    // буфер вывода, встал бы намертво, пока мы ждём его завершения, — и
    // накопленное остаётся доступным даже после kill по таймауту, ровно как у
    // питоновского `communicate()`.
    let stdout = child.stdout.take().map(read_to_string);
    let stderr = child.stderr.take().map(read_to_string);

    if let Some(mut stdin) = child.stdin.take() {
        // Ошибку записи глотаем: процесс мог уже умереть, и тогда пользователю
        // важен его вывод, а не EPIPE. `communicate()` ведёт себя так же.
        let _ = stdin.write_all(format!("{code}\n").as_bytes()).await;
        // `stdin` уходит из области видимости и закрывается: без EOF adb ждал
        // бы ввода дальше и упёрся бы в таймаут на каждом спаривании.
    }

    let timed_out = tokio::time::timeout(PAIR_TIMEOUT, child.wait())
        .await
        .is_err();
    if timed_out {
        let _ = child.kill().await;
    }

    // Python сливал stderr в stdout ещё на уровне труб (`stderr=STDOUT`) и
    // получал естественное чередование. Портируемого аналога в tokio нет, так
    // что склеиваем: сначала stdout, потом stderr. На контракт это не влияет —
    // `Successfully paired` adb пишет в stdout.
    let mut output = String::new();
    for task in [stdout, stderr].into_iter().flatten() {
        // Трубу мог унаследовать поднятый демон adb — тогда чтение до EOF не
        // кончится никогда, хотя сам `adb pair` давно вышел. Python здесь тоже
        // ограничивался пятью секундами.
        if let Ok(Ok(text)) = tokio::time::timeout(PAIR_READ_TIMEOUT, task).await {
            output.push_str(&text);
        }
    }

    if timed_out {
        output.push_str(&format!(
            "\n[adb pair timed out after {} seconds]",
            PAIR_TIMEOUT.as_secs()
        ));
    }

    Ok(output)
}

/// Вычитывает трубу в фоне. Ошибки чтения дают пустую строку: полупрочитанный
/// вывод полезнее отсутствующего.
fn read_to_string<R>(mut source: R) -> tokio::task::JoinHandle<String>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = source.read_to_end(&mut buffer).await;
        String::from_utf8_lossy(&buffer).into_owned()
    })
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
    fn parses_wifi_ip_from_ip_addr_output() {
        let real = "\
23: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default qlen 3000
    link/ether ac:5f:3e:11:22:33 brd ff:ff:ff:ff:ff:ff
    inet6 fe80::ae5f:3eff:fe11:2233/64 scope link
    inet 192.168.1.42/24 brd 192.168.1.255 scope global wlan0
";
        assert_eq!(parse_wifi_ip(real).as_deref(), Some("192.168.1.42"));

        // CRLF: на Windows вывод adb приходит с ним, а `lines()` его снимает.
        assert_eq!(
            parse_wifi_ip("    inet 10.0.0.7/24 scope global wlan0\r\n").as_deref(),
            Some("10.0.0.7")
        );
        // Адрес без маски adb не печатает, но терять его на этом не стоит.
        assert_eq!(
            parse_wifi_ip("    inet 10.0.0.8 scope global wlan0").as_deref(),
            Some("10.0.0.8")
        );
        // Только IPv6 — адреса нет, эндпоинт отдаст null.
        assert_eq!(parse_wifi_ip("    inet6 fe80::1/64 scope link"), None);
        assert_eq!(parse_wifi_ip(""), None);
        // Строка без второго поля не должна ронять разбор — в Python её
        // отсеивала проверка `len(parts) >= 2`.
        assert_eq!(parse_wifi_ip("inet "), None);
    }

    #[test]
    fn combined_output_repeats_python_concatenation() {
        let output = AdbOutput {
            code: 1,
            stdout: "failed to connect\n".to_string(),
            stderr: "error: no devices\n".to_string(),
            timed_out: false,
        };
        assert!(!output.success());
        assert_eq!(output.combined(), "failed to connect\nerror: no devices\n");
    }

    /// `-s <serial>` встаёт перед командой, а не после: `adb tcpip 5555 -s X`
    /// adb не понимает.
    #[test]
    fn serial_goes_before_the_command() {
        assert_eq!(with_serial(None, &["tcpip", "5555"]), ["tcpip", "5555"]);
        assert_eq!(
            with_serial(Some("R58M123"), &["tcpip", "5555"]),
            ["-s", "R58M123", "tcpip", "5555"]
        );
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
