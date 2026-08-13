#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod connection;
mod devices;
mod error;
mod events;
mod i18n;
mod paths;
mod recording;
mod scrcpy;
mod service;
mod signal;
mod tools;

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{Manager, RunEvent, WindowEvent};

struct BackendState(Mutex<Option<Child>>);

static API_TOKEN: OnceLock<String> = OnceLock::new();

/// Общий секрет между лаунчером и бэкендом.
///
/// Бэкенд получает его через окружение и требует в каждом запросе, а фронтенд
/// забирает командой `mkdsc_api_token` — то есть токен никогда не покидает
/// приложение и посторонняя вкладка браузера его не узнает.
fn api_token() -> &'static str {
    API_TOKEN.get_or_init(|| {
        std::env::var("MKDSC_API_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string())
    })
}

#[tauri::command]
fn mkdsc_api_token() -> String {
    api_token().to_string()
}

/// Прячет консольное окно дочернего процесса на Windows.
///
/// Живёт здесь, а не в `tools.rs`: спавн внешних процессов — забота лаунчера,
/// а `devices.rs` берёт эту же функцию через `command.as_std_mut()`, чтобы
/// поведение не разъехалось между бэкендом и adb.
#[cfg(windows)]
pub(crate) fn set_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn set_no_window(_command: &mut Command) {}

fn configure_backend_stdio(command: &mut Command, data_dir: &std::path::Path) {
    if cfg!(debug_assertions) {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        return;
    }

    let log_dir = data_dir.join("logs");
    if std::fs::create_dir_all(&log_dir).is_ok() {
        let log_path = log_dir.join("backend.log");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let err_file = file.try_clone().ok();
            command.stdout(Stdio::from(file));
            if let Some(err_file) = err_file {
                command.stderr(Stdio::from(err_file));
            } else {
                command.stderr(Stdio::null());
            }
            return;
        }
    }

    command.stdout(Stdio::null()).stderr(Stdio::null());
}

fn log_launcher_event(data_dir: &std::path::Path, message: &str) {
    let log_dir = data_dir.join("logs");
    if std::fs::create_dir_all(&log_dir).is_ok() {
        let log_path = log_dir.join("backend.log");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(file, "[launcher:{}] {}", ts, message);
        }
    }
}

/// Запускает Python-бэкенд: сначала собранный бинарь, иначе `tauri_backend.py`.
///
/// `MKDSC_CONFIG_READONLY=1` переводит бэкенд в режим «только чтение конфига».
/// `config.json` принадлежит лаунчеру (см. `config.rs`), и второй писатель не
/// повредил бы файл, а просто затирал бы чужие правки целиком: каждый пишет
/// свою версию, прочитанную до правки соседа. Читать конфиг Python
/// продолжает — оттуда берутся настройки scrcpy, записи и путей. Standalone
/// (`python web_panel.py`) переменной не видит и работает как раньше.
fn spawn_backend(app: &tauri::AppHandle) -> Result<Child, Box<dyn std::error::Error>> {
    let base_dir = paths::base_dir(app);
    let data_dir = paths::data_dir(app);

    std::fs::create_dir_all(data_dir)?;
    log_launcher_event(
        data_dir,
        &format!(
            "base_dir={} data_dir={} prefer_python={}",
            base_dir.display(),
            data_dir.display(),
            cfg!(debug_assertions) || std::env::var_os("MKDSC_FORCE_PYTHON").is_some()
        ),
    );

    let prefer_python =
        cfg!(debug_assertions) || std::env::var_os("MKDSC_FORCE_PYTHON").is_some();

    let spawn_binary = || -> Result<Option<Child>, Box<dyn std::error::Error>> {
        let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
        let backend_name = format!("mkdsc-backend{exe_suffix}");
        let backend_candidates = [
            base_dir.join("bin").join(&backend_name),
            base_dir.join("src-tauri").join("bin").join(&backend_name),
        ];

        let backend_path = backend_candidates
            .into_iter()
            .find(|path| path.exists());

        if let Some(path) = backend_path {
            log_launcher_event(
                data_dir,
                &format!("backend binary found at {}", path.display()),
            );
            let mut command = Command::new(path);
            command
                .current_dir(base_dir)
                .env("MKDSC_BASE_DIR", base_dir)
                .env("MKDSC_DATA_DIR", data_dir)
                .env("MKDSC_HOST", "127.0.0.1")
                .env("MKDSC_PORT", "6969")
                .env("MKDSC_AUTO_OPEN", "0")
                .env("MKDSC_CONFIG_READONLY", "1")
                .env("MKDSC_API_TOKEN", api_token());
            configure_backend_stdio(&mut command, data_dir);
            set_no_window(&mut command);

            return Ok(Some(command.spawn()?));
        }

        log_launcher_event(
            data_dir,
            "backend binary not found (bin/mkdsc-backend)",
        );
        Ok(None)
    };

    let spawn_python = || -> Result<Child, Box<dyn std::error::Error>> {
        let python = if let Ok(value) = std::env::var("MKDSC_PYTHON") {
            value
        } else {
            let venv_python = if cfg!(windows) {
                base_dir.join(".venv").join("Scripts").join("python.exe")
            } else {
                base_dir.join(".venv").join("bin").join("python")
            };
            if venv_python.exists() {
                venv_python.to_string_lossy().to_string()
            } else if cfg!(windows) {
                "python".to_string()
            } else {
                "python3".to_string()
            }
        };

        let script_path = base_dir.join("tauri_backend.py");
        if !script_path.exists() {
            let message = format!("backend script not found: {}", script_path.display());
            log_launcher_event(data_dir, &message);
            return Err(message.into());
        }

        let mut command = Command::new(python);
        command
            .arg(script_path)
            .current_dir(base_dir)
            .env("MKDSC_BASE_DIR", base_dir)
            .env("MKDSC_DATA_DIR", data_dir)
            .env("MKDSC_HOST", "127.0.0.1")
            .env("MKDSC_PORT", "6969")
            .env("MKDSC_AUTO_OPEN", "0")
            .env("MKDSC_CONFIG_READONLY", "1")
            .env("MKDSC_API_TOKEN", api_token());
        configure_backend_stdio(&mut command, data_dir);
        set_no_window(&mut command);

        Ok(command.spawn()?)
    };

    if !prefer_python {
        if let Some(child) = spawn_binary()? {
            return Ok(child);
        }
    }

    match spawn_python() {
        Ok(child) => Ok(child),
        Err(err) => {
            if prefer_python {
                if let Some(child) = spawn_binary()? {
                    return Ok(child);
                }
            }
            Err(err)
        }
    }
}

fn stop_backend(state: &BackendState) {
    if let Ok(mut guard) = state.0.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            mkdsc_api_token,
            api::api_devices,
            api::api_devices_save,
            api::api_devices_delete,
            api::api_config,
            api::api_config_full,
            api::api_config_update,
            api::api_config_replace,
            api::api_i18n,
            api::api_presets,
            api::api_presets_save,
            api::api_presets_delete,
            api::api_connect,
            api::api_disconnect,
            api::api_pair,
            api::api_tcpip,
            api::api_adb_restart,
            api::api_connection_auto_detect,
            api::api_connection_auto_switch,
            api::api_connection_metrics,
            api::api_connection_metrics_device,
            api::api_service_commands,
            api::api_service_run,
            api::api_service_custom,
            api::api_scrcpy_launch,
            api::api_recording_status,
            api::api_recording_start,
            api::api_recording_stop
        ])
        .manage(BackendState(Mutex::new(None)))
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            // Signed, in-place updates with a real relaunch. Replaces the old
            // hand-rolled updater that copied a source zipball over the
            // install directory.
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let child = spawn_backend(app.handle())?;
            let state = app.state::<BackendState>();
            if let Ok(mut guard) = state.0.lock() {
                *guard = Some(child);
            }

            // Поток устройств поднимается один раз на всё приложение, а не на
            // окно: перезагрузка страницы (F5) не должна плодить вторую задачу
            // и удваивать опрос adb.
            events::spawn(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { .. },
                    ..
                }
                | RunEvent::ExitRequested { .. }
                | RunEvent::Exit => {
                    // Настройки возвращаются до остановки бэкенда: они лежат на
                    // чужом устройстве, и если этого не сделать здесь, то
                    // `stay_on_while_plugged_in=3` останется на телефоне
                    // навсегда — фоновая задача, ждущая выхода scrcpy, уйдёт
                    // вместе с рантаймом. Очередь дренируется, поэтому три
                    // события подряд ничего не повторят.
                    scrcpy::restore_on_exit();

                    let state = app_handle.state::<BackendState>();
                    stop_backend(&state);
                }
                _ => {}
            }
        });
}
