#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use tauri::{Manager, RunEvent, WindowEvent};

struct BackendState(Mutex<Option<Child>>);

#[cfg(windows)]
fn set_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn set_no_window(_command: &mut Command) {}

fn resolve_base_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    if let Some(explicit) = std::env::var_os("MKDSC_BASE_DIR") {
        return std::path::PathBuf::from(explicit);
    }

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir);
    }
    if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.clone());
        if let Some(parent) = dir.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_dir.parent() {
        candidates.push(parent.to_path_buf());
    }

    for base in candidates {
        if base.join("tauri_backend.py").exists() {
            return base;
        }
    }

    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn resolve_data_dir(app: &tauri::AppHandle, base_dir: &std::path::Path) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .ok()
        .unwrap_or_else(|| base_dir.to_path_buf())
}

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

fn spawn_backend(app: &tauri::AppHandle) -> Result<Child, Box<dyn std::error::Error>> {
    let base_dir = resolve_base_dir(app);
    let data_dir = resolve_data_dir(app, &base_dir);

    std::fs::create_dir_all(&data_dir)?;

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
            let mut command = Command::new(path);
            command
                .current_dir(&base_dir)
                .env("MKDSC_BASE_DIR", &base_dir)
                .env("MKDSC_DATA_DIR", &data_dir)
                .env("MKDSC_HOST", "127.0.0.1")
                .env("MKDSC_PORT", "6969")
                .env("MKDSC_AUTO_OPEN", "0");
            configure_backend_stdio(&mut command, &data_dir);
            set_no_window(&mut command);

            return Ok(Some(command.spawn()?));
        }

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

        let mut command = Command::new(python);
        command
            .arg(script_path)
            .current_dir(&base_dir)
            .env("MKDSC_BASE_DIR", &base_dir)
            .env("MKDSC_DATA_DIR", &data_dir)
            .env("MKDSC_HOST", "127.0.0.1")
            .env("MKDSC_PORT", "6969")
            .env("MKDSC_AUTO_OPEN", "0");
        configure_backend_stdio(&mut command, &data_dir);
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
        .manage(BackendState(Mutex::new(None)))
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            let child = spawn_backend(&app.handle())?;
            let state = app.state::<BackendState>();
            if let Ok(mut guard) = state.0.lock() {
                *guard = Some(child);
            }
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
                    let state = app_handle.state::<BackendState>();
                    stop_backend(&state);
                }
                _ => {}
            }
        });
}
