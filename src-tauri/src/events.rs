//! Поток обновлений списка устройств — замена WebSocket `/ws`.
//!
//! Одна фоновая задача опрашивает adb и рассылает событие `devices_update` с
//! тем же payload, что слал сокет (`{type, devices, timestamp}`). Благодаря
//! этому обработчик в `App.tsx` переехал один в один: поменялся только способ
//! подписки.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::devices::{self, ConnectedDevice};
use crate::tools;

/// Имя события. Совпадает с полем `type` в payload — как в WS-сообщении.
pub const DEVICES_UPDATE: &str = "devices_update";

/// Как часто опрашивается adb — тот же интервал, что был в WS-цикле.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// «Пульс»: даже если список не менялся, событие уходит не реже этого срока.
///
/// Слать его безусловно каждые три секунды — лишние перерисовки на ровном
/// месте. Но и молчать нельзя: индикатор online/offline в боковой панели
/// считает «онлайн» как «событие приходило недавно» и на неподвижном списке
/// погас бы. Фронтенд ждёт событие дольше — `DEVICE_STREAM_TIMEOUT_MS` в
/// `frontend/src/lib/events.ts`; за тем, чтобы константы не разъехались,
/// следит тест `heartbeat_matches_frontend_constant`.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// Что и когда ушло клиенту прошлый раз.
struct Sent {
    devices: Vec<ConnectedDevice>,
    at: Instant,
}

/// Замок вокруг пары «опрос — рассылка».
///
/// Он же сериализует опросы: фоновая задача и немедленное обновление после
/// `connect` могут прийтись на один момент, и без замка более старый ответ
/// adb успел бы отправиться вторым — UI откатился бы к устаревшему списку.
///
/// `tokio::sync::Mutex`, а не `std::sync::Mutex`: замок держится через `await`.
static SENT: OnceLock<tokio::sync::Mutex<Option<Sent>>> = OnceLock::new();

fn sent() -> &'static tokio::sync::Mutex<Option<Sent>> {
    SENT.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Поднимает фоновую задачу потока устройств.
///
/// Вызывается **один раз** из `setup()`, а не на каждое открытие окна:
/// перезагрузка страницы (F5) не должна плодить вторую копию задачи и удваивать
/// опрос adb. Задача живёт до выхода из приложения, отменять её незачем.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            refresh(&app).await;
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

/// Опрашивает adb и рассылает событие, если список изменился или пришло время
/// пульса.
///
/// Вызывается и фоновой задачей, и командами подключения: после `connect`,
/// `disconnect` и `tcpip` список в UI должен обновиться сразу, а не на
/// следующем такте через три секунды.
pub async fn refresh(app: &AppHandle) {
    let mut last = sent().lock().await;

    let connected = match tools::adb_path(app) {
        Some(adb) => match devices::list_connected(&adb).await {
            Ok(connected) => connected,
            // Сбой опроса — не повод слать пустой список: пользователь увидел
            // бы «устройств нет» вместо «связи нет». Пропускаем такт; если
            // сбой затянется, индикатор погаснет сам и фронтенд вернётся к
            // запасному HTTP-поллингу.
            Err(_) => return,
        },
        // adb ещё качается — шлём пустой список, а не ошибку: ровно так вёл
        // себя и `/ws`, пока `app.state.adb_path` не выставлен.
        None => Vec::new(),
    };

    if !should_emit(last.as_ref(), &connected) {
        return;
    }

    let payload = json!({
        "type": DEVICES_UPDATE,
        "devices": &connected,
        "timestamp": crate::api::now_iso(),
    });

    // Ошибку рассылки глотаем: окно могло уже закрыться, а задача живёт до
    // выхода из приложения и должна пережить это молча.
    let _ = app.emit(DEVICES_UPDATE, payload);

    *last = Some(Sent {
        devices: connected,
        at: Instant::now(),
    });
}

/// Слать ли событие: список изменился либо пульс просрочен.
fn should_emit(last: Option<&Sent>, connected: &[ConnectedDevice]) -> bool {
    match last {
        Some(previous) => previous.devices != connected || previous.at.elapsed() >= HEARTBEAT,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(serial: &str) -> ConnectedDevice {
        ConnectedDevice {
            serial: serial.to_string(),
            status: "device".to_string(),
        }
    }

    fn sent_at(devices: &[ConnectedDevice], ago: Duration) -> Sent {
        Sent {
            devices: devices.to_vec(),
            at: Instant::now()
                .checked_sub(ago)
                .expect("часы монотонны и заведомо старше нескольких секунд"),
        }
    }

    #[test]
    fn first_poll_always_emits() {
        assert!(should_emit(None, &[]));
    }

    #[test]
    fn unchanged_list_stays_quiet_until_the_heartbeat() {
        let devices = [device("emulator-5554")];

        let fresh = sent_at(&devices, Duration::from_secs(3));
        assert!(!should_emit(Some(&fresh), &devices));

        let stale = sent_at(&devices, HEARTBEAT + Duration::from_secs(1));
        assert!(should_emit(Some(&stale), &devices), "пульс не сработал");
    }

    #[test]
    fn changed_list_emits_immediately() {
        let before = [device("emulator-5554")];
        let fresh = sent_at(&before, Duration::from_secs(1));

        assert!(should_emit(Some(&fresh), &[]));
        assert!(should_emit(
            Some(&fresh),
            &[device("emulator-5554"), device("192.168.1.5:5555")]
        ));
        // Статус — часть списка: `unauthorized` → `device` обязано доехать до
        // UI, иначе кнопки останутся заблокированными до следующего пульса.
        assert!(should_emit(
            Some(&fresh),
            &[ConnectedDevice {
                serial: "emulator-5554".to_string(),
                status: "unauthorized".to_string(),
            }]
        ));
    }

    /// Пульс в Rust и ожидание события на фронте обязаны сходиться.
    ///
    /// Стоит удлинить пульс здесь и забыть про константу там — индикатор
    /// начнёт мигать «оффлайн» на неподвижном списке, а вместе с ним вернётся
    /// запасной HTTP-поллинг и двойной опрос adb.
    #[test]
    fn heartbeat_matches_frontend_constant() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri лежит в корне репозитория")
            .join("frontend")
            .join("src")
            .join("lib")
            .join("events.ts");
        let source = std::fs::read_to_string(&path).expect("events.ts на месте");

        let declared = source
            .lines()
            .find_map(|line| {
                let (_, value) = line.split_once("DEVICE_STREAM_HEARTBEAT_MS = ")?;
                value.trim().trim_end_matches(';').parse::<u128>().ok()
            })
            .expect("константа DEVICE_STREAM_HEARTBEAT_MS найдена");

        assert_eq!(declared, HEARTBEAT.as_millis());
    }
}
