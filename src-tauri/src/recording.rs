//! Состояние записи экрана: старт, статус, остановка.
//!
//! Питоновский оригинал — `start_recording`, `recording_status`,
//! `stop_recording`, `_recording_watch` и `_stop_recording_process` в
//! `mkdsc/web/server.py`. Формы ответов повторяются один в один.
//!
//! Запись — единственное место порта, где живёт долгоживущий процесс и общее
//! изменяемое состояние, поэтому здесь стоит помнить три вещи:
//!
//! 1. Процесс нельзя убивать сразу — файл останется без индекса (`signal.rs`).
//! 2. Вывод процесса обязательно вычитывать: заполнив буфер трубы, он встанет
//!    намертво, и остановка ничего не дождётся.
//! 3. Штатную остановку нельзя записывать в `last_error` — иначе интерфейс
//!    показал бы ошибку после каждой удачной записи.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Child;
use tokio::sync::watch;

use crate::error::ApiError;
use crate::{api, config, paths, scrcpy, signal};

/// Сколько ждём, пока scrcpy сам допишет контейнер.
///
/// 45 секунд — намеренно много: на длинной записи дописывание индекса не
/// мгновенное, а поспешить здесь значит отдать пользователю битый файл.
const STOP_TIMEOUT: Duration = Duration::from_secs(45);

/// Сколько ждём после принудительного убийства — как `wait(timeout=5)`.
const KILL_TIMEOUT: Duration = Duration::from_secs(5);

/// Пауза перед проверкой, что процесс не умер сразу.
///
/// scrcpy часто падает в первые миллисекунды — например, когда устройство
/// отвалилось. Без проверки запись «начиналась» и тут же исчезала, а
/// пользователь видел успех.
const EARLY_EXIT_CHECK: Duration = Duration::from_millis(400);

/// Сколько ждём вывод процесса, умершего на старте, — как `communicate(1)`.
const EARLY_OUTPUT_WAIT: Duration = Duration::from_secs(1);

/// Сколько последних строк вывода держим.
const OUTPUT_TAIL: usize = 40;

/// Сколько строк уезжает в `last_error`.
const ERROR_TAIL: usize = 10;

/// Предупреждение о том, что процесс пришлось убить.
pub const FORCED_STOP_WARNING: &str = "notification_recording_forced_stop";

/// Идущая запись.
///
/// Процесс здесь не лежит: им владеет задача-наблюдатель, единственная, кто его
/// ждёт. Две «ждущие» стороны на один `Child` не сходятся по владению, а
/// наблюдатель нужен в любом случае — scrcpy может закрыться и сам.
struct Session {
    pid: u32,
    started_at: String,
    output_path: PathBuf,
    settings: Value,
    /// Останавливаемся штатно — наблюдатель не должен счесть это ошибкой.
    stopping: bool,
    /// Взводится наблюдателем, когда процесс завершился.
    ///
    /// `watch`, а не `Notify`: у него есть текущее значение, поэтому ожидающий
    /// не проспит уже случившееся завершение.
    finished: watch::Receiver<bool>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Последняя неудача записи — переживает саму сессию, её показывает интерфейс.
static LAST_ERROR: Mutex<Option<Value>> = Mutex::new(None);

/// Хвост вывода процесса, общий для читателей stdout и stderr.
#[derive(Clone, Default)]
struct OutputTail(Arc<Mutex<Vec<String>>>);

impl OutputTail {
    fn push(&self, line: String) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(line);
            if lines.len() > OUTPUT_TAIL {
                lines.remove(0);
            }
        }
    }

    /// Последние `count` строк одним текстом.
    fn tail(&self, count: usize) -> String {
        let Ok(lines) = self.0.lock() else {
            return String::new();
        };
        let start = lines.len().saturating_sub(count);
        lines[start..].join("\n")
    }
}

/// Идёт ли запись прямо сейчас.
pub fn is_active() -> bool {
    SESSION.lock().is_ok_and(|session| session.is_some())
}

/// Зеркалит `GET /api/recording/status` — форма из `_recording_status_payload`.
pub fn status() -> Value {
    let session = SESSION.lock().ok();
    let Some(session) = session.as_ref().and_then(|guard| guard.as_ref()) else {
        let mut payload = json!({"active": false});
        if let Some(error) = last_error() {
            payload["last_error"] = error;
        }
        return payload;
    };

    let mut payload = json!({
        "active": true,
        "pid": session.pid,
        "started_at": session.started_at,
        "output_path": session.output_path.to_string_lossy(),
    });
    // Настройки уезжают плоско, рядом с остальными полями, — так их читает
    // интерфейс.
    if let Some(settings) = session.settings.as_object() {
        for (key, value) in settings {
            payload[key] = value.clone();
        }
    }
    payload
}

fn last_error() -> Option<Value> {
    LAST_ERROR.lock().ok()?.clone()
}

fn set_last_error(error: Option<Value>) {
    if let Ok(mut last) = LAST_ERROR.lock() {
        *last = error;
    }
}

/// Зеркалит `POST /api/recording/start`.
///
/// Возвращает `{success, pid, started_at, output_path, filename, settings,
/// warning_key, failed_settings}`.
///
/// Ошибки: 409 — запись уже идёт; 400 — каталог не создать или формат не тот;
/// 500 — scrcpy не запустился или умер сразу.
pub async fn start(
    app: &AppHandle,
    scrcpy_bin: &Path,
    adb: &Path,
    data: &Map<String, Value>,
) -> Result<Value, ApiError> {
    if is_active() {
        return Err(ApiError::new(409, "Recording already active"));
    }

    set_last_error(None);

    let config = config::load(app)?;
    let plan = scrcpy::RecordingPlan::resolve(data, &config, paths::recordings_dir(app, &config))?;

    std::fs::create_dir_all(&plan.output_dir).map_err(|err| ApiError::new(400, err.to_string()))?;
    if !plan.output_dir.is_dir() {
        return Err(ApiError::new(400, "Output path is not a directory"));
    }

    let file_name = plan.file_name(&scrcpy::file_timestamp());
    let output_path = plan.output_dir.join(&file_name);
    let args = plan.args(&output_path);

    let (restore, failed_settings) = scrcpy::apply_device_settings(
        adb,
        plan.stay_awake,
        plan.show_touches,
        plan.serial.as_deref(),
    )
    .await?;

    let mut child = match scrcpy::spawn_recorder(scrcpy_bin, &args) {
        Ok(child) => child,
        Err(error) => {
            scrcpy::restore_device_settings(adb, &restore, plan.serial.as_deref()).await;
            return Err(error);
        }
    };
    let pid = child.id().unwrap_or_default();

    let tail = OutputTail::default();
    let readers = drain_output(&mut child, &tail);

    // Ранний выход: scrcpy падает в первые миллисекунды, если устройство
    // отвалилось, а без проверки пользователь увидел бы «запись пошла».
    tokio::time::sleep(EARLY_EXIT_CHECK).await;
    if let Ok(Some(exit)) = child.try_wait() {
        // Дочитываем то, что процесс успел сказать: это единственное
        // объяснение, которое получит пользователь.
        let _ = tokio::time::timeout(EARLY_OUTPUT_WAIT, readers).await;
        scrcpy::restore_device_settings(adb, &restore, plan.serial.as_deref()).await;

        let output = tail.tail(OUTPUT_TAIL);
        let detail = if output.is_empty() {
            format!(
                "Recording failed to start (exit code {}).",
                exit.code().unwrap_or(-1)
            )
        } else {
            output
        };
        set_last_error(Some(json!({
            "exit_code": exit.code(),
            "timestamp": api::now_iso(),
            "output": detail,
        })));
        return Err(ApiError::internal(detail));
    }

    let started_at = api::now_iso();
    let settings = plan.settings();
    let ticket = scrcpy::defer_restore(adb, plan.serial.as_deref(), restore);
    let (finished_tx, finished_rx) = watch::channel(false);

    if let Ok(mut session) = SESSION.lock() {
        *session = Some(Session {
            pid,
            started_at: started_at.clone(),
            output_path: output_path.clone(),
            settings: settings.clone(),
            stopping: false,
            finished: finished_rx,
        });
    }

    watch_process(child, pid, tail, ticket, finished_tx);

    Ok(json!({
        "success": true,
        "pid": pid,
        "started_at": started_at,
        "output_path": output_path.to_string_lossy(),
        "filename": file_name,
        "settings": settings,
        "warning_key": plan.warning_key,
        "failed_settings": failed_settings,
    }))
}

/// Зеркалит `POST /api/recording/stop`.
///
/// Возвращает `{success, graceful, output_path, warning_key}`; `warning_key`
/// непустой, если scrcpy пришлось убить, — тогда файл может не открыться.
/// Когда записи нет — `{success: false, message}`, как в Python.
pub async fn stop() -> Value {
    // Замок отпускается до ожидания: иначе `status` завис бы на все 45 секунд.
    let Some((pid, output_path, mut finished)) = begin_stop() else {
        return json!({"success": false, "message": "No active recording"});
    };

    signal::request_stop(pid);

    // Успех проверяется только выходом процесса. Код возврата
    // `GenerateConsoleCtrlEvent` для этого не годится — он бывает
    // «успешным» и тогда, когда событие не дошло (см. `signal.rs`).
    let graceful = tokio::time::timeout(STOP_TIMEOUT, finished.wait_for(|done| *done))
        .await
        .is_ok();

    if !graceful {
        signal::force_kill(pid);
        let _ = tokio::time::timeout(KILL_TIMEOUT, finished.wait_for(|done| *done)).await;
    }

    json!({
        "success": true,
        "graceful": graceful,
        "output_path": output_path,
        "warning_key": if graceful { Value::Null } else { json!(FORCED_STOP_WARNING) },
    })
}

/// Помечает сессию останавливаемой и отдаёт всё нужное для ожидания.
fn begin_stop() -> Option<(u32, Value, watch::Receiver<bool>)> {
    let mut guard = SESSION.lock().ok()?;
    let session = guard.as_mut()?;
    session.stopping = true;
    Some((
        session.pid,
        json!(session.output_path.to_string_lossy()),
        session.finished.clone(),
    ))
}

/// Вычитывает обе трубы процесса в общий хвост.
///
/// Возвращает задачу, завершающуюся вместе с трубами: её ждёт проверка раннего
/// выхода, чтобы не потерять последнее сообщение упавшего scrcpy.
fn drain_output(child: &mut Child, tail: &OutputTail) -> tauri::async_runtime::JoinHandle<()> {
    // Python сливал stderr в stdout ещё на уровне труб; портируемого аналога в
    // tokio нет, поэтому читаем обе в один буфер — порядок строк между потоками
    // при этом не гарантирован, но для диагностики важен состав, а не порядок.
    let stdout = child.stdout.take().map(|pipe| read_lines(pipe, tail.clone()));
    let stderr = child.stderr.take().map(|pipe| read_lines(pipe, tail.clone()));

    tauri::async_runtime::spawn(async move {
        for reader in [stdout, stderr].into_iter().flatten() {
            let _ = reader.await;
        }
    })
}

fn read_lines<R>(pipe: R, tail: OutputTail) -> tauri::async_runtime::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if !line.is_empty() {
                tail.push(line);
            }
        }
    })
}

/// Ждёт завершения scrcpy, чистит сессию и возвращает настройки устройства.
fn watch_process(
    mut child: Child,
    pid: u32,
    tail: OutputTail,
    ticket: Option<u64>,
    finished: watch::Sender<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let code = child.wait().await.ok().and_then(|status| status.code());

        // Сессия снимается до сигнала «завершился»: иначе опрос статуса сразу
        // после остановки успел бы показать запись всё ещё идущей.
        finish_session(pid, code, &tail);
        let _ = finished.send(true);

        // Настройки возвращаются последними: они уходят на устройство и могут
        // занять секунды, а ответ на остановку ждать этого не обязан.
        if let Some(ticket) = ticket {
            scrcpy::run_pending(ticket).await;
        }
    });
}

/// Снимает сессию и, если завершение не было штатным, запоминает ошибку.
fn finish_session(pid: u32, code: Option<i32>, tail: &OutputTail) {
    let Ok(mut guard) = SESSION.lock() else {
        return;
    };
    // Сессию мог сменить кто-то другой — тогда она не наша, и трогать её нельзя.
    let stopping = match guard.as_ref() {
        Some(session) if session.pid == pid => session.stopping,
        _ => return,
    };
    *guard = None;
    drop(guard);

    if stopping || code == Some(0) {
        return;
    }

    let mut error = json!({"exit_code": code, "timestamp": api::now_iso()});
    let output = tail.tail(ERROR_TAIL);
    if !output.is_empty() {
        error["output"] = json!(output);
    }
    set_last_error(Some(error));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_tail_keeps_only_the_last_lines() {
        let tail = OutputTail::default();
        for index in 0..OUTPUT_TAIL + 5 {
            tail.push(format!("line {index}"));
        }

        let kept = tail.tail(OUTPUT_TAIL);
        assert_eq!(kept.lines().count(), OUTPUT_TAIL);
        // Именно хвост: диагностика упавшего процесса живёт в последних строках.
        assert!(kept.starts_with("line 5"));
        assert!(kept.ends_with(&format!("line {}", OUTPUT_TAIL + 4)));

        assert_eq!(tail.tail(2), format!("line {}\nline {}", OUTPUT_TAIL + 3, OUTPUT_TAIL + 4));
        assert_eq!(OutputTail::default().tail(10), "");
    }

    /// Без активной сессии статус — это `{active: false}` плюс последняя ошибка.
    #[test]
    fn status_without_session_reports_last_error() {
        set_last_error(None);
        assert_eq!(status(), json!({"active": false}));

        set_last_error(Some(json!({"exit_code": 1, "timestamp": "t"})));
        let payload = status();
        assert_eq!(payload["active"], json!(false));
        assert_eq!(payload["last_error"]["exit_code"], json!(1));

        set_last_error(None);
    }

    #[test]
    fn stop_without_session_answers_like_python() {
        let answer = tauri::async_runtime::block_on(stop());
        assert_eq!(answer["success"], json!(false));
        assert_eq!(answer["message"], json!("No active recording"));
    }
}
