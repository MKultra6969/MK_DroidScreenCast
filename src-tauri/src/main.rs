#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod bootstrap;
mod config;
mod connection;
mod devices;
mod error;
mod events;
mod files;
mod i18n;
mod install;
mod logs;
mod paths;
mod recording;
mod scrcpy;
mod screenshots;
mod service;
mod signal;
mod tools;
mod updater;

use std::io::Write;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{RunEvent, WindowEvent};

/// Прячет консольное окно дочернего процесса на Windows.
///
/// Живёт здесь, а не в `tools.rs`: спавн внешних процессов — забота лаунчера,
/// а `devices.rs` берёт эту же функцию через `command.as_std_mut()`, чтобы
/// поведение не разъехалось между модулями.
#[cfg(windows)]
pub(crate) fn set_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn set_no_window(_command: &mut Command) {}

/// Пишет строку в `logs/launcher.log`.
///
/// Единственная диагностика того, что происходит до появления окна: сбой
/// подготовки инструментов виден в интерфейсе, а всё, что раньше, — только
/// здесь. Лог уезжает в архив `POST /api/logs/export` вместе с остальными.
pub(crate) fn log_launcher_event(data_dir: &std::path::Path, message: &str) {
    let log_dir = data_dir.join("logs");
    if std::fs::create_dir_all(&log_dir).is_ok() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("launcher.log"))
        {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default();
            let _ = writeln!(file, "[launcher:{stamp}] {message}");
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            api::api_devices,
            api::api_devices_save,
            api::api_devices_delete,
            api::api_config,
            api::api_config_full,
            api::api_config_update,
            api::api_config_replace,
            api::api_i18n,
            api::api_bootstrap_status,
            api::api_update_check,
            api::api_logs_export,
            api::api_files_list,
            api::api_files_delete,
            api::api_files_mkdir,
            api::api_files_move,
            api::api_files_read,
            api::api_files_write,
            api::api_files_pull,
            api::api_files_upload,
            api::api_screenshots,
            api::api_screenshots_take,
            api::api_screenshots_save,
            api::api_screenshots_caption,
            api::api_screenshots_delete,
            api::api_screenshots_delete_many,
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
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            // Signed, in-place updates with a real relaunch. Replaces the old
            // hand-rolled updater that copied a source zipball over the
            // install directory.
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let handle = app.handle().clone();
            log_launcher_event(paths::data_dir(&handle), "starting");

            // Подготовка инструментов — в фоне: окно должно открыться сразу, а
            // не после полутора сотен мегабайт загрузки. Прогресс интерфейс
            // забирает командой `api_bootstrap_status`.
            let installer = handle.clone();
            tauri::async_runtime::spawn(async move {
                install::ensure_tools(&installer).await;
            });

            // Поток устройств поднимается один раз на всё приложение, а не на
            // окно: перезагрузка страницы (F5) не должна плодить вторую задачу
            // и удваивать опрос adb.
            events::spawn(handle);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            match event {
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { .. },
                    ..
                }
                | RunEvent::ExitRequested { .. }
                | RunEvent::Exit => {
                    // Настройки возвращаются до выхода: они лежат на чужом
                    // устройстве, и если этого не сделать здесь, то
                    // `stay_on_while_plugged_in=3` останется на телефоне
                    // навсегда — фоновая задача, ждущая выхода scrcpy, уйдёт
                    // вместе с рантаймом. Очередь дренируется, поэтому три
                    // события подряд ничего не повторят.
                    scrcpy::restore_on_exit();
                }
                _ => {}
            }
        });
}
