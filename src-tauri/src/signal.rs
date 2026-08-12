//! Мягкая остановка дочернего процесса — от неё зависит целость записи.
//!
//! scrcpy дописывает контейнер MP4/MKV (moov-атом) только при штатном
//! завершении. Убитый жёстко процесс оставляет файл, который не открывается
//! ничем, поэтому запись останавливается сигналом, а `TerminateProcess` —
//! только как крайняя мера после долгого ожидания.
//!
//! # Почему на Windows это не одна строка
//!
//! `GenerateConsoleCtrlEvent` работает **через консоль**: отправитель обязан
//! быть подключён к той же консоли, что и адресат. Замеры на живой системе
//! (Windows 11, Rust-лаунчер в GUI-подсистеме):
//!
//! | Как запущен потомок | Результат сигнала |
//! |---|---|
//! | без флагов | ошибка 6 (`ERROR_INVALID_HANDLE`), не отправлен |
//! | `CREATE_NEW_PROCESS_GROUP` | ошибка 6, не отправлен |
//! | `CREATE_NO_WINDOW` + группа | **вызов вернул успех, событие не дошло** |
//! | `CREATE_NO_WINDOW` + группа + `AttachConsole` | доставлен, процесс дописал файл |
//!
//! Третья строка — главная ловушка: функция возвращает `TRUE`, а событие уходит
//! в никуда, потому что `CREATE_NO_WINDOW` даёт потомку **его собственную**
//! безоконную консоль, а не общую с нами. Поэтому успех остановки проверяется
//! только по тому, что процесс действительно завершился, и никогда — по коду
//! возврата этой функции.
//!
//! У Python-бэкенда проблемы нет, хотя код у него тот же: PyInstaller собирает
//! его **консольным** приложением (`scripts/build_tauri_backend.py` не передаёт
//! `--noconsole`), а `CREATE_NO_WINDOW` даёт консольному приложению безоконную
//! консоль. scrcpy запускается оттуда без собственных консольных флагов и эту
//! консоль наследует, так что сигнал доходит. Лаунчер на Rust собран с
//! `windows_subsystem = "windows"` — консоли у него нет вообще, и дословный
//! перенос кода Python сломал бы остановку записи, причём молча.
//!
//! Отсюда решение: scrcpy запускается с `CREATE_NO_WINDOW |
//! CREATE_NEW_PROCESS_GROUP`, а лаунчер на время сигнала подсаживается в его
//! консоль через `AttachConsole`. Консольного окна не появляется нигде, и
//! глобальной консоли у приложения не заводится — в отличие от варианта с
//! `AllocConsole`, который мигнул бы окном на старте.
//!
//! На POSIX ничего этого не нужно: `kill(pid, SIGINT)` доходит всегда.

/// Просит процесс завершиться штатно.
///
/// Возвращает `true`, если сигнал удалось отправить. Это **не** значит, что
/// процесс его получил и тем более что он завершился, — за этим следит
/// вызывающий, дожидаясь выхода процесса.
pub fn request_stop(pid: u32) -> bool {
    imp::request_stop(pid)
}

/// Убивает процесс без шансов дописать файл.
///
/// Крайняя мера: запись после неё, скорее всего, не откроется. Звать по номеру
/// процесса безопасно ровно потому, что его `Child` ещё не пожат — пока это не
/// сделано, система номер не переиспользует.
pub fn force_kill(pid: u32) -> bool {
    imp::force_kill(pid)
}

#[cfg(windows)]
mod imp {
    use std::sync::{Mutex, MutexGuard};

    const CTRL_BREAK_EVENT: u32 = 1;
    /// `AttachConsole(ATTACH_PARENT_PROCESS)` — вернуться к консоли родителя.
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    /// `AttachConsole` так отвечает, когда консоль у нас уже есть.
    const ERROR_ACCESS_DENIED: u32 = 5;

    /// `OpenProcess` — минимальные права, нужные для `TerminateProcess`.
    const PROCESS_TERMINATE: u32 = 0x0001;

    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn FreeConsole() -> i32;
        fn GenerateConsoleCtrlEvent(ctrl_event: u32, process_group_id: u32) -> i32;
        fn GetLastError() -> u32;
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> isize;
        fn TerminateProcess(process: isize, exit_code: u32) -> i32;
        fn CloseHandle(object: isize) -> i32;
    }

    /// Консоль у процесса ровно одна, поэтому подсадки сериализуются: две
    /// одновременные отцепили бы друг у друга консоль на полпути.
    static CONSOLE: Mutex<()> = Mutex::new(());

    fn lock() -> MutexGuard<'static, ()> {
        // Отравленный замок здесь не беда: под ним нет данных, которые могло бы
        // испортить чужой паникой.
        CONSOLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn request_stop(pid: u32) -> bool {
        let _guard = lock();

        unsafe {
            let mut borrowed_own_console = false;

            if AttachConsole(pid) == 0 {
                // `GetLastError` читается только после нуля: после успешного
                // вызова там лежит код от какого-то предыдущего, и доверять ему
                // нельзя.
                if GetLastError() != ERROR_ACCESS_DENIED {
                    // Процесса уже нет либо консоли у него нет — слать некуда.
                    return false;
                }

                // Своя консоль есть — так бывает в отладочной сборке, запущенной
                // из терминала. Отпускаем её на время сигнала и возвращаем
                // обратно, иначе отладочный вывод после первой же остановки
                // уходил бы в никуда.
                borrowed_own_console = true;
                FreeConsole();
                if AttachConsole(pid) == 0 {
                    AttachConsole(ATTACH_PARENT_PROCESS);
                    return false;
                }
            }

            let sent = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) != 0;

            FreeConsole();
            if borrowed_own_console {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }

            sent
        }
    }

    pub fn force_kill(pid: u32) -> bool {
        unsafe {
            let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if process == 0 {
                return false;
            }
            let killed = TerminateProcess(process, 1) != 0;
            CloseHandle(process);
            killed
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// Номера сигналов одинаковы на всех поддерживаемых POSIX-платформах.
    const SIGINT: i32 = 2;
    const SIGKILL: i32 = 9;

    // Ради двух вызовов тащить в зависимости `libc` незачем — тем же приёмом
    // в `tools.rs` обходится `shutil.which`.
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    pub fn request_stop(pid: u32) -> bool {
        // SIGINT, а не SIGTERM: scrcpy опирается на обработчик SDL, который
        // превращает его в SDL_QUIT и даёт дописать контейнер. Это же шлёт
        // Python (`_stop_recording_process`).
        unsafe { kill(pid as i32, SIGINT) == 0 }
    }

    pub fn force_kill(pid: u32) -> bool {
        unsafe { kill(pid as i32, SIGKILL) == 0 }
    }
}
