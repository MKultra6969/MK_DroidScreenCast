/**
 * Единственная точка выхода фронтенда в бэкенд.
 *
 * Бэкенд теперь один — Rust за Tauri IPC. HTTP-слоя нет: путь и метод
 * превращаются в имя команды по таблице ниже, а результат заворачивается в
 * настоящий `Response`. Благодаря этому 40+ мест вызова `apiFetch` работают с
 * ним как с ответом `fetch` — `.ok`, `.status`, `readJson()`, `.headers.get()`.
 */

type AppWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

/**
 * Tauri v2 injects `__TAURI_INTERNALS__` unconditionally; `__TAURI__` only
 * appears with `app.withGlobalTauri`, which this app does not enable — so the
 * old check was always false and the desktop code paths never ran.
 */
export const isTauri = () =>
  typeof window !== 'undefined' && Boolean((window as AppWindow).__TAURI_INTERNALS__);

type ApiFetchOptions = RequestInit & { timeoutMs?: number };

type IpcRoute = {
  method: string;
  /** Шаблон пути. Сегменты вида `{name}` попадают в `params`. */
  pattern: string;
  command: string;
};

/**
 * Таблица маршрутов: путь и метод — имя команды.
 *
 * Имена достались от REST-эндпоинтов, которыми это было до переноса
 * (`GET /api/devices` → `api_devices`), и пока такими и остаются: переименовать
 * их в «нормальные» команды — отдельная работа, которая тронет каждую страницу.
 */
const IPC_ROUTES: IpcRoute[] = [
  { method: 'GET', pattern: '/api/devices', command: 'api_devices' },
  { method: 'POST', pattern: '/api/devices/save', command: 'api_devices_save' },
  { method: 'DELETE', pattern: '/api/devices/{ip}/{port}', command: 'api_devices_delete' },
  { method: 'GET', pattern: '/api/config', command: 'api_config' },
  { method: 'POST', pattern: '/api/config', command: 'api_config_update' },
  { method: 'PUT', pattern: '/api/config', command: 'api_config_replace' },
  { method: 'GET', pattern: '/api/config/full', command: 'api_config_full' },
  { method: 'GET', pattern: '/api/i18n', command: 'api_i18n' },
  { method: 'GET', pattern: '/api/bootstrap/status', command: 'api_bootstrap_status' },
  { method: 'GET', pattern: '/api/update/check', command: 'api_update_check' },
  { method: 'POST', pattern: '/api/logs/export', command: 'api_logs_export' },
  { method: 'GET', pattern: '/api/presets', command: 'api_presets' },
  { method: 'POST', pattern: '/api/presets', command: 'api_presets_save' },
  { method: 'DELETE', pattern: '/api/presets/{name}', command: 'api_presets_delete' },
  { method: 'POST', pattern: '/api/connect', command: 'api_connect' },
  { method: 'POST', pattern: '/api/disconnect', command: 'api_disconnect' },
  { method: 'POST', pattern: '/api/pair', command: 'api_pair' },
  { method: 'POST', pattern: '/api/tcpip', command: 'api_tcpip' },
  { method: 'POST', pattern: '/api/adb/restart', command: 'api_adb_restart' },
  { method: 'POST', pattern: '/api/connection/auto-detect', command: 'api_connection_auto_detect' },
  { method: 'POST', pattern: '/api/connection/auto-switch', command: 'api_connection_auto_switch' },
  { method: 'GET', pattern: '/api/connection/metrics', command: 'api_connection_metrics' },
  { method: 'GET', pattern: '/api/connection/metrics/{serial}', command: 'api_connection_metrics_device' },
  { method: 'GET', pattern: '/api/service/commands', command: 'api_service_commands' },
  // `custom` — до шаблона: иначе он уехал бы в команду как имя предопределённой
  // и вернул бы 404 вместо выполнения. Совпадение ищется по порядку.
  { method: 'POST', pattern: '/api/service/custom', command: 'api_service_custom' },
  { method: 'POST', pattern: '/api/service/{command_name}', command: 'api_service_run' },
  { method: 'POST', pattern: '/api/scrcpy/launch', command: 'api_scrcpy_launch' },
  { method: 'GET', pattern: '/api/recording/status', command: 'api_recording_status' },
  { method: 'POST', pattern: '/api/recording/start', command: 'api_recording_start' },
  // Остановка ждёт, пока scrcpy допишет файл, — до 45 секунд. Своего таймаута у
  // `invoke` нет, и это как раз то, что нужно: оборванная остановка оставила бы
  // запись без индекса.
  { method: 'POST', pattern: '/api/recording/stop', command: 'api_recording_stop' },
  { method: 'GET', pattern: '/api/files/list', command: 'api_files_list' },
  { method: 'DELETE', pattern: '/api/files/delete', command: 'api_files_delete' },
  { method: 'POST', pattern: '/api/files/mkdir', command: 'api_files_mkdir' },
  { method: 'POST', pattern: '/api/files/move', command: 'api_files_move' },
  { method: 'GET', pattern: '/api/files/read', command: 'api_files_read' },
  { method: 'POST', pattern: '/api/files/write', command: 'api_files_write' },
  { method: 'POST', pattern: '/api/files/pull', command: 'api_files_pull' },
  // Тело здесь — `{source}` с путём к файлу на диске: содержимое файла через
  // IPC не передать, поэтому загрузка отдаёт путь, а `adb push` читает файл сам.
  { method: 'POST', pattern: '/api/files/upload', command: 'api_files_upload' },
  { method: 'GET', pattern: '/api/screenshots', command: 'api_screenshots' },
  { method: 'DELETE', pattern: '/api/screenshots', command: 'api_screenshots_delete_many' },
  { method: 'POST', pattern: '/api/screenshots/take', command: 'api_screenshots_take' },
  { method: 'DELETE', pattern: '/api/screenshots/{id}', command: 'api_screenshots_delete' },
  { method: 'POST', pattern: '/api/screenshots/{id}/save', command: 'api_screenshots_save' },
  { method: 'PUT', pattern: '/api/screenshots/{id}/caption', command: 'api_screenshots_caption' },
];

type IpcMatch = {
  command: string;
  params: Record<string, string>;
};

const matchIpcRoute = (method: string, pathname: string): IpcMatch | null => {
  const segments = pathname.split('/');

  for (const route of IPC_ROUTES) {
    if (route.method !== method) continue;

    const patternSegments = route.pattern.split('/');
    if (patternSegments.length !== segments.length) continue;

    const params: Record<string, string> = {};
    const matched = patternSegments.every((expected, index) => {
      const actual = segments[index];
      if (!expected.startsWith('{') || !expected.endsWith('}')) {
        return expected === actual;
      }
      // Пустой сегмент (`//`) — не значение параметра, а битый путь.
      if (!actual) return false;
      // Вызывающий код кодирует сегменты через `encodeURIComponent`;
      // в команду они должны прийти раскодированными.
      params[expected.slice(1, -1)] = decodeURIComponent(actual);
      return true;
    });

    if (matched) return { command: route.command, params };
  }

  return null;
};

/** Тело запроса как JSON. Роутер получает его уже разобранным. */
const parseIpcBody = (body: BodyInit | null | undefined): unknown => {
  if (typeof body !== 'string' || !body) return null;
  try {
    return JSON.parse(body);
  } catch {
    return null;
  }
};

const jsonResponse = (payload: unknown, status: number) =>
  new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });

/** Выполняет команду и заворачивает результат в настоящий `Response`. */
const invokeIpcRoute = async (
  match: IpcMatch,
  query: Record<string, string>,
  body: unknown,
): Promise<Response> => {
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    const payload = await invoke<unknown>(match.command, { params: match.params, query, body });
    return jsonResponse(payload, 200);
  } catch (error) {
    // Rust отдаёт ошибку структурой `{status, detail}`. Разворачиваем её в
    // ответ с тем же статусом и телом `{"detail": ...}` — это та форма,
    // которую разбирают `readJson()` и `fileErrorMessage()`.
    const failure = error as { status?: unknown; detail?: unknown } | null;
    const status = typeof failure?.status === 'number' ? failure.status : 500;
    const detail = typeof failure?.detail === 'string' ? failure.detail : String(error);
    return jsonResponse({ detail }, status);
  }
};

/**
 * Отправляет запрос по адресу вида `/api/...`.
 *
 * `timeoutMs` больше ни на что не влияет — таймауты задаёт Rust, — но остаётся
 * в сигнатуре, чтобы не править вызывающий код ради одного удалённого поля.
 */
export const apiFetch = (path: string, init: ApiFetchOptions = {}): Promise<Response> => {
  let url: URL;
  try {
    // Разбор через URL, а не регулярками: нужно отделить `pathname` от
    // `searchParams`, и делать это вручную — верный способ ошибиться на
    // экранировании. База фиктивная, `path` всегда относительный.
    url = new URL(path, 'http://mkdsc.invalid');
  } catch {
    return Promise.resolve(jsonResponse({ detail: `Malformed request path: ${path}` }, 400));
  }

  const method = (init.method || 'GET').toUpperCase();
  const match = matchIpcRoute(method, url.pathname);
  if (!match) {
    // Раньше здесь был запасной путь в `fetch`. Сети больше нет: непрописанный
    // маршрут — это ошибка сборки, и молчать о ней хуже, чем ответить пятисоткой
    // с внятным текстом.
    return Promise.resolve(
      jsonResponse({ detail: `No IPC route for ${method} ${url.pathname}` }, 500),
    );
  }

  const query: Record<string, string> = {};
  // Повторяющиеся ключи схлопываются: списочных параметров в API нет.
  url.searchParams.forEach((value, key) => {
    query[key] = value;
  });

  return invokeIpcRoute(match, query, parseIpcBody(init.body));
};

export const readJson = async <T = any>(response: Response): Promise<T> => {
  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return response.json() as Promise<T>;
  }
  const text = await response.text();
  return { error: text || `HTTP ${response.status}` } as T;
};
