//! Выбор подключения по латентности — порт `mkdsc/web/connection_optimizer.py`.
//!
//! Четыре эндпоинта: замер по одному устройству, замер по всем, рекомендация и
//! переключение на Wi-Fi. Полезны они ровно тем, что решают за пользователя,
//! запускать scrcpy по USB или по сети.
//!
//! Здесь важнее всего таймауты. `POST /api/connection/auto-switch` в худшем
//! случае делает `tcpip`, `connect` и до шести замеров латентности — без
//! таймаута на каждом вызове одна недоступная железка вешала бы команду
//! целиком, а фронтенд ждёт её перед запуском scrcpy.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::devices::{self, ConnectedDevice};
use crate::error::ApiError;

/// Выше этого Wi-Fi не рекомендуется никогда — `MAX_WIFI_LATENCY_MS`.
const MAX_WIFI_LATENCY_MS: f64 = 120.0;

/// Wi-Fi выигрывает, только если заметно быстрее USB — `WIFI_BETTER_RATIO`.
const WIFI_BETTER_RATIO: f64 = 0.9;

/// USB предпочитается, пока проигрывает не больше 20%: кабель стабильнее.
const USB_TOLERANCE: f64 = 1.2;

/// Сколько раз меряем латентность, чтобы усреднить.
const LATENCY_ITERATIONS: usize = 3;

/// Таймаут одного замера — `subprocess.run(..., timeout=5)`.
const LATENCY_TIMEOUT: Duration = Duration::from_secs(5);

/// Таймаут `tcpip` и `connect` — `_run_adb(..., timeout=10)`.
const SWITCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Порт подключения по сети по умолчанию.
const DEFAULT_PORT: &str = "5555";

/// Метрики одного подключения — `ConnectionMetrics`.
#[derive(Debug, Clone)]
pub struct Metrics {
    serial: String,
    /// `usb` или `wifi` — фронтенд подставляет это в параметры scrcpy.
    connection_type: &'static str,
    /// Средняя латентность в миллисекундах; `None` — устройство не ответило.
    latency_ms: Option<f64>,
    error: Option<&'static str>,
}

impl Metrics {
    fn is_available(&self) -> bool {
        self.latency_ms.is_some()
    }

    /// Латентность для сравнений: недоступное устройство хуже любого доступного.
    fn latency(&self) -> f64 {
        self.latency_ms.unwrap_or(f64::INFINITY)
    }

    /// Форма из `asdict(ConnectionMetrics)` — порядок полей тот же.
    ///
    /// Недоступное устройство отдаёт `latency_ms: null`. В Python там
    /// `float('inf')`, и это никогда не доезжало до фронтенда: `JSONResponse`
    /// сериализует с `allow_nan=False` и падает пятисоткой на первом же
    /// бесконечном значении. То есть `auto-switch` при живом Wi-Fi и мёртвом
    /// USB отвечал ошибкой вместо рекомендации.
    fn to_value(&self) -> Value {
        json!({
            "serial": self.serial,
            "connection_type": self.connection_type,
            "latency_ms": self.latency_ms,
            "is_available": self.is_available(),
            "error": self.error,
        })
    }

    /// Поле `recommended` ответов `auto-detect` и `auto-switch`.
    ///
    /// Фронтенд разбирает из него `serial` и `type` — менять имена нельзя.
    fn to_recommendation(&self) -> Value {
        json!({
            "serial": self.serial,
            "type": self.connection_type,
            "latency_ms": self.latency_ms,
        })
    }
}

/// USB-подключение отличается от сетевого отсутствием `:` в серийнике.
fn is_usb(serial: &str) -> bool {
    !serial.contains(':')
}

/// Средняя латентность `adb shell echo ping` по трём проходам.
///
/// `None`, если ни один проход не удался: устройство отвалилось, не
/// авторизовано или просто не отвечает. Любая ошибка внутри прохода — пропуск
/// прохода, а не ошибка замера: так же ведёт себя `except (...): pass` в Python.
async fn measure_latency(adb: &Path, serial: &str) -> Option<f64> {
    let mut samples = Vec::with_capacity(LATENCY_ITERATIONS);

    for _ in 0..LATENCY_ITERATIONS {
        let started = Instant::now();
        // Замеряется полный цикл вместе с запуском процесса — именно его и
        // платит scrcpy на каждой команде, ради этого замер и делается.
        let output = devices::run_adb(
            adb,
            &["-s", serial, "shell", "echo", "ping"],
            LATENCY_TIMEOUT,
        )
        .await;

        if matches!(&output, Ok(output) if output.success()) {
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    if samples.is_empty() {
        return None;
    }
    Some(samples.iter().sum::<f64>() / samples.len() as f64)
}

/// Метрики одного подключения — `get_connection_metrics`.
pub async fn metrics(adb: &Path, serial: &str) -> Metrics {
    let latency = measure_latency(adb, serial).await;

    Metrics {
        serial: serial.to_string(),
        connection_type: if is_usb(serial) { "usb" } else { "wifi" },
        // Округление до сотых — как `round(latency, 2)`: миллисекунды с
        // четырнадцатью знаками после запятой в ответе никому не нужны.
        latency_ms: latency.map(|value| (value * 100.0).round() / 100.0),
        error: latency.is_none().then_some("Device not responding"),
    }
}

/// Зеркалит `GET /api/connection/metrics/{serial}` — метрики одного устройства.
pub async fn device_metrics(adb: &Path, serial: &str) -> Value {
    metrics(adb, serial).await.to_value()
}

/// Зеркалит `GET /api/connection/metrics` — метрики всех подключённых.
pub async fn all_metrics(adb: &Path) -> Result<Value, ApiError> {
    let measured = measure_all(adb, &devices::list_connected(adb).await?).await;

    Ok(json!({
        "metrics": measured.iter().map(Metrics::to_value).collect::<Vec<_>>(),
        "device_count": measured.len(),
    }))
}

/// Зеркалит `POST /api/connection/auto-detect`.
///
/// Меряет все подключения и рекомендует лучшее. При прочих равных выигрывает
/// USB: кабель стабильнее, и проигрыш в пределах 20% того не стоит.
pub async fn auto_detect(adb: &Path) -> Result<Value, ApiError> {
    let connected = devices::list_connected(adb).await?;
    if connected.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "No devices connected",
            "all_metrics": [],
        }));
    }

    let measured = measure_all(adb, &connected).await;
    let all_metrics = measured.iter().map(Metrics::to_value).collect::<Vec<_>>();

    let available: Vec<&Metrics> = measured.iter().filter(|item| item.is_available()).collect();
    if available.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "No available devices",
            "all_metrics": all_metrics,
        }));
    }

    let best = fastest(available.iter().copied()).expect("список не пуст");
    let best_usb = fastest(
        available
            .iter()
            .copied()
            .filter(|item| item.connection_type == "usb"),
    );
    let best = match best_usb {
        Some(usb) if usb.latency() <= best.latency() * USB_TOLERANCE => usb,
        _ => best,
    };

    Ok(json!({
        "success": true,
        "recommended": best.to_recommendation(),
        "all_metrics": all_metrics,
        "device_count": {
            "total": measured.len(),
            "usb": count_of(&measured, "usb"),
            "wifi": count_of(&measured, "wifi"),
        },
    }))
}

/// Зеркалит `POST /api/connection/auto-switch`.
///
/// Находит для устройства пару «USB + Wi-Fi», при необходимости поднимая
/// сетевое подключение (`tcpip` + `connect`), и рекомендует то, что быстрее.
/// Wi-Fi выигрывает только с запасом: переключение туда стоит переподключения,
/// и делать его ради пяти процентов незачем.
pub async fn auto_switch(adb: &Path, payload: &Map<String, Value>) -> Result<Value, ApiError> {
    let target = payload
        .get("serial")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|serial| !serial.is_empty());
    // `str(payload.get("port") or "5555").strip() or "5555"`: и отсутствие, и
    // любое ложное значение (пустая строка, 0, null) дают порт по умолчанию.
    let port = payload
        .get("port")
        .filter(|port| crate::config::is_truthy(port))
        .map(crate::config::python_str)
        .map(|port| port.trim().to_string())
        .filter(|port| !port.is_empty())
        .unwrap_or_else(|| DEFAULT_PORT.to_string());

    let mut connected = devices::list_connected(adb).await?;
    if connected.is_empty() {
        return Ok(json!({"success": false, "error": "No devices connected"}));
    }

    if let Some(target) = target {
        if !connected.iter().any(|device| device.serial == target) {
            return Ok(json!({"success": false, "error": "Device not connected"}));
        }
    }

    // Без явного серийника берём первое USB-устройство, а если их нет — просто
    // первое: переключать на Wi-Fi имеет смысл именно то, что висит на кабеле.
    let Some(target) = target.map(str::to_string).or_else(|| {
        connected
            .iter()
            .find(|device| is_usb(&device.serial))
            .or_else(|| connected.first())
            .map(|device| device.serial.clone())
    }) else {
        return Ok(json!({"success": false, "error": "No valid device serial found"}));
    };

    let usb_serial = is_usb(&target).then(|| target.clone());
    let mut wifi_serial = (!is_usb(&target)).then(|| target.clone());
    let mut attempted_tcpip = false;

    if let Some(usb) = usb_serial.as_deref() {
        let ip = devices::wifi_ip(adb, Some(usb)).await?;
        wifi_serial = find_wifi_serial(&connected, ip.as_deref(), &port);

        // Адрес есть, а подключения по нему ещё нет — поднимаем его сами.
        if let Some(ip) = ip.filter(|_| wifi_serial.is_none()) {
            attempted_tcpip = true;
            // Ошибки глотаются: устройство могло не дать `tcpip`, и это не повод
            // ронять весь запрос — ниже всё равно останется USB.
            let _ = devices::run_adb(adb, &["-s", usb, "tcpip", &port], SWITCH_TIMEOUT).await;
            let _ = devices::run_adb(adb, &["connect", &format!("{ip}:{port}")], SWITCH_TIMEOUT)
                .await;

            connected = devices::list_connected(adb).await?;
            wifi_serial = find_wifi_serial(&connected, Some(&ip), &port);
        }
    }

    let usb_metric = match usb_serial.as_deref() {
        Some(serial) => Some(metrics(adb, serial).await),
        None => None,
    };
    let wifi_metric = match wifi_serial.as_deref() {
        Some(serial) => Some(metrics(adb, serial).await),
        None => None,
    };

    let recommended = recommend(usb_metric.as_ref(), wifi_metric.as_ref());
    let Some(recommended) = recommended else {
        return Ok(json!({"success": false, "error": "No responsive devices"}));
    };

    Ok(json!({
        "success": true,
        "recommended": recommended.to_recommendation(),
        "metrics": {
            "usb": usb_metric.as_ref().map(Metrics::to_value),
            "wifi": wifi_metric.as_ref().map(Metrics::to_value),
        },
        "attempted_tcpip": attempted_tcpip,
    }))
}

/// Что рекомендовать из пары «USB + Wi-Fi».
fn recommend<'a>(usb: Option<&'a Metrics>, wifi: Option<&'a Metrics>) -> Option<&'a Metrics> {
    let wifi_available = wifi.filter(|metric| metric.is_available());
    let usb_available = usb.filter(|metric| metric.is_available());

    if let Some(wifi) = wifi_available {
        // Wi-Fi берётся, только если он и сам по себе быстрый, и заметно
        // быстрее USB (либо USB просто нет).
        let wifi_wins = wifi.latency() <= MAX_WIFI_LATENCY_MS
            && usb_available.is_none_or(|usb| wifi.latency() < usb.latency() * WIFI_BETTER_RATIO);
        if wifi_wins {
            return Some(wifi);
        }
    }

    // Иначе — USB, а если и его нет, то хоть какой-то ответивший Wi-Fi.
    usb_available.or(wifi_available)
}

/// Серийник сетевого подключения к тому же адресу — `_find_wifi_serial`.
fn find_wifi_serial(
    connected: &[ConnectedDevice],
    ip: Option<&str>,
    port: &str,
) -> Option<String> {
    let ip = ip?;
    let prefix = format!("{ip}:");
    let suffix = format!(":{port}");

    connected
        .iter()
        .find(|device| device.serial.starts_with(&prefix) && device.serial.ends_with(&suffix))
        .map(|device| device.serial.clone())
}

/// Меряет все устройства по очереди.
///
/// Именно по очереди, а не параллельно: замер конкурирует сам с собой через
/// один демон adb, и три параллельных `echo ping` мерили бы очередь, а не связь.
async fn measure_all(adb: &Path, connected: &[ConnectedDevice]) -> Vec<Metrics> {
    let mut measured = Vec::with_capacity(connected.len());
    for device in connected {
        measured.push(metrics(adb, &device.serial).await);
    }
    measured
}

/// Самое быстрое подключение из перечисленных.
fn fastest<'a>(items: impl Iterator<Item = &'a Metrics>) -> Option<&'a Metrics> {
    // `min_by` с `partial_cmp`: NaN тут взяться неоткуда — латентность либо
    // измерена, либо `None`, что даёт бесконечность.
    items.min_by(|left, right| {
        left.latency()
            .partial_cmp(&right.latency())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn count_of(measured: &[Metrics], connection_type: &str) -> usize {
    measured
        .iter()
        .filter(|item| item.connection_type == connection_type)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(serial: &str, latency: Option<f64>) -> Metrics {
        Metrics {
            serial: serial.to_string(),
            connection_type: if is_usb(serial) { "usb" } else { "wifi" },
            latency_ms: latency,
            error: latency.is_none().then_some("Device not responding"),
        }
    }

    fn device(serial: &str) -> ConnectedDevice {
        ConnectedDevice {
            serial: serial.to_string(),
            status: "device".to_string(),
        }
    }

    #[test]
    fn usb_is_a_serial_without_a_port() {
        assert!(is_usb("R58M123ABCD"));
        assert!(is_usb("emulator-5554"));
        assert!(!is_usb("192.168.1.5:5555"));
    }

    /// Wi-Fi выигрывает только с запасом: сам быстрый и заметно быстрее USB.
    #[test]
    fn wifi_wins_only_with_a_margin() {
        let usb = metric("R58M123", Some(50.0));

        // 44 < 50 * 0.9 — берём Wi-Fi.
        let fast = metric("192.168.1.5:5555", Some(44.0));
        assert_eq!(
            recommend(Some(&usb), Some(&fast)).map(|m| m.serial.as_str()),
            Some("192.168.1.5:5555")
        );

        // 46 быстрее USB, но не на 10% — остаёмся на кабеле.
        let close = metric("192.168.1.5:5555", Some(46.0));
        assert_eq!(
            recommend(Some(&usb), Some(&close)).map(|m| m.serial.as_str()),
            Some("R58M123")
        );

        // Быстрее USB, но сам по себе медленный — тоже мимо.
        let slow_usb = metric("R58M123", Some(400.0));
        let slow_wifi = metric("192.168.1.5:5555", Some(200.0));
        assert_eq!(
            recommend(Some(&slow_usb), Some(&slow_wifi)).map(|m| m.serial.as_str()),
            Some("R58M123")
        );
        // Случай выше держится на том, что 200 мс выше порога.
        const { assert!(MAX_WIFI_LATENCY_MS < 200.0) };
    }

    /// Мёртвая половина пары не мешает выбрать живую.
    #[test]
    fn unavailable_sides_are_skipped() {
        let dead_usb = metric("R58M123", None);
        let wifi = metric("192.168.1.5:5555", Some(300.0));

        // USB не отвечает — берём Wi-Fi, даже медленный.
        assert_eq!(
            recommend(Some(&dead_usb), Some(&wifi)).map(|m| m.serial.as_str()),
            Some("192.168.1.5:5555")
        );

        // Wi-Fi не отвечает — остаёмся на USB.
        let usb = metric("R58M123", Some(50.0));
        let dead_wifi = metric("192.168.1.5:5555", None);
        assert_eq!(
            recommend(Some(&usb), Some(&dead_wifi)).map(|m| m.serial.as_str()),
            Some("R58M123")
        );

        // Никто не отвечает — рекомендации нет.
        assert!(recommend(Some(&dead_usb), Some(&dead_wifi)).is_none());
        assert!(recommend(None, None).is_none());
        // Только Wi-Fi, USB не искали.
        assert_eq!(
            recommend(None, Some(&wifi)).map(|m| m.serial.as_str()),
            Some("192.168.1.5:5555")
        );
    }

    /// При прочих равных — USB: кабель стабильнее сети.
    #[test]
    fn usb_wins_ties_within_the_tolerance() {
        let items = [
            metric("192.168.1.5:5555", Some(50.0)),
            metric("R58M123", Some(59.0)),
        ];
        let best = fastest(items.iter()).expect("есть");
        assert_eq!(best.serial, "192.168.1.5:5555");

        // 59 <= 50 * 1.2 — предпочитаем USB.
        let best_usb = fastest(items.iter().filter(|m| m.connection_type == "usb")).expect("есть");
        assert!(best_usb.latency() <= best.latency() * USB_TOLERANCE);

        // 61 уже нет.
        let far = metric("R58M123", Some(61.0));
        assert!(far.latency() > best.latency() * USB_TOLERANCE);
    }

    #[test]
    fn wifi_serial_is_matched_by_address_and_port() {
        let connected = [
            device("R58M123"),
            device("192.168.1.5:5555"),
            device("192.168.1.50:5555"),
        ];

        assert_eq!(
            find_wifi_serial(&connected, Some("192.168.1.5"), "5555").as_deref(),
            Some("192.168.1.5:5555")
        );
        // Другой порт — не то подключение.
        assert_eq!(find_wifi_serial(&connected, Some("192.168.1.5"), "5556"), None);
        // Похожий адрес не совпадает: `192.168.1.5:` — не префикс `192.168.1.50:`.
        assert_eq!(
            find_wifi_serial(&connected, Some("192.168.1.50"), "5555").as_deref(),
            Some("192.168.1.50:5555")
        );
        assert_eq!(find_wifi_serial(&connected, None, "5555"), None);
        assert_eq!(find_wifi_serial(&connected, Some("10.0.0.1"), "5555"), None);
    }

    /// Форма ответа читается фронтендом — поля и их имена менять нельзя.
    #[test]
    fn metric_shape_mirrors_python() {
        let available = metric("R58M123", Some(12.34)).to_value();
        assert_eq!(
            available,
            json!({
                "serial": "R58M123",
                "connection_type": "usb",
                "latency_ms": 12.34,
                "is_available": true,
                "error": null,
            })
        );

        let dead = metric("192.168.1.5:5555", None).to_value();
        assert_eq!(dead["is_available"], json!(false));
        assert_eq!(dead["latency_ms"], Value::Null);
        assert_eq!(dead["error"], json!("Device not responding"));

        let recommendation = metric("R58M123", Some(12.34)).to_recommendation();
        assert_eq!(recommendation["serial"], json!("R58M123"));
        assert_eq!(recommendation["type"], json!("usb"));
    }
}
