/**
 * Поток обновлений списка устройств.
 *
 * Замена WebSocket `/ws`: Rust рассылает событие `devices_update` с тем же
 * payload, что слал сокет (`{type, devices, timestamp}`), поэтому обработчик в
 * `App.tsx` не менялся — поменялся только способ подписки.
 */

/** Имя события. Совпадает с полем `type` в payload — как в WS-сообщении. */
const DEVICES_UPDATE = 'devices_update';

/**
 * Как часто Rust шлёт «пульс», даже если список устройств не менялся
 * (`HEARTBEAT` в `src-tauri/src/events.rs`). Совпадение проверяет тест
 * `heartbeat_matches_frontend_constant`.
 */
export const DEVICE_STREAM_HEARTBEAT_MS = 15000;

/**
 * Сколько ждём событие, прежде чем считать поток вставшим.
 *
 * У событий Tauri нет понятия «соединение», которое можно спросить, поэтому
 * «онлайн» — это «событие приходило недавно». Два пульса плюс запас: одного
 * мало, любая задержка гасила бы индикатор и включала запасной HTTP-поллинг
 * на ровном месте.
 */
export const DEVICE_STREAM_TIMEOUT_MS = DEVICE_STREAM_HEARTBEAT_MS * 2 + 2000;

/** Payload события — та же форма, что была у WS-сообщения. */
export type DevicesUpdate = {
  type?: string;
  devices?: unknown;
  timestamp?: string;
};

/**
 * Подписывается на поток устройств и возвращает функцию отписки.
 *
 * Отписку обязательно вызывать в cleanup эффекта: подписки Tauri не снимаются
 * сами, и после нескольких перезагрузок страницы обработчик отработал бы
 * столько же раз на каждое событие.
 */
export const listenDevicesUpdate = async (
  onUpdate: (payload: DevicesUpdate) => void
): Promise<() => void> => {
  const { listen } = await import('@tauri-apps/api/event');
  return listen<DevicesUpdate>(DEVICES_UPDATE, (event) => onUpdate(event.payload));
};
