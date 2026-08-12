const envBase = (import.meta.env.VITE_API_BASE || '').replace(/\/$/, '');
const API_BASE = envBase || '';

const TOKEN_HEADER = 'X-MKDSC-Token';

type AppWindow = Window & {
  __MKDSC_TOKEN__?: string;
  __TAURI_INTERNALS__?: unknown;
};

/**
 * Tauri v2 injects `__TAURI_INTERNALS__` unconditionally; `__TAURI__` only
 * appears with `app.withGlobalTauri`, which this app does not enable — so the
 * old check was always false and the desktop code paths never ran.
 */
export const isTauri = () =>
  typeof window !== 'undefined' && Boolean((window as AppWindow).__TAURI_INTERNALS__);

// In the web panel the backend injects the token into the page it serves.
let apiToken =
  typeof window !== 'undefined' ? String((window as AppWindow).__MKDSC_TOKEN__ || '') : '';
let tokenPromise: Promise<string> | null = null;

export const getApiToken = () => apiToken;

/** In the desktop build the token comes from the Rust launcher over IPC. */
export const ensureApiToken = async (): Promise<string> => {
  if (apiToken) return apiToken;
  if (!isTauri()) return '';
  if (!tokenPromise) {
    tokenPromise = (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        apiToken = String((await invoke<string>('mkdsc_api_token')) || '');
      } catch (error) {
        console.error('api token error', error);
      }
      return apiToken;
    })();
  }
  return tokenPromise;
};

export const apiUrl = (path: string) => `${API_BASE}${path}`;

/** For <img src> and <a href>, where no request headers can be attached. */
export const apiUrlWithToken = (path: string) => {
  const url = apiUrl(path);
  if (!apiToken) return url;
  const joiner = url.includes('?') ? '&' : '?';
  return `${url}${joiner}token=${encodeURIComponent(apiToken)}`;
};

const withAuth = (init: RequestInit): RequestInit => {
  if (!apiToken) return init;
  const headers = new Headers(init.headers || {});
  headers.set(TOKEN_HEADER, apiToken);
  return { ...init, headers };
};

type ApiFetchOptions = RequestInit & { timeoutMs?: number };

type IpcRoute = {
  method: string;
  /** Шаблон пути. Сегменты вида `{name}` попадают в `params`. */
  pattern: string;
  command: string;
};

/**
 * Эндпоинты, уехавшие с HTTP на Tauri IPC.
 *
 * Команда называется по методу и пути (`GET /api/devices` → `api_devices`),
 * а ответ Rust собирает в ту же форму, что отдаёт FastAPI. Всё, чего в этой
 * таблице нет, роутер не трогает — такие пути молча уходят в `fetch`, и
 * непереехавшие эндпоинты продолжают работать.
 */
const IPC_ROUTES: IpcRoute[] = [
  { method: 'GET', pattern: '/api/devices', command: 'api_devices' },
  { method: 'POST', pattern: '/api/devices/save', command: 'api_devices_save' },
  { method: 'DELETE', pattern: '/api/devices/{ip}/{port}', command: 'api_devices_delete' },
  { method: 'GET', pattern: '/api/config', command: 'api_config' },
  { method: 'POST', pattern: '/api/config', command: 'api_config_update' },
  { method: 'PUT', pattern: '/api/config', command: 'api_config_replace' },
  { method: 'GET', pattern: '/api/config/full', command: 'api_config_full' },
  { method: 'GET', pattern: '/api/presets', command: 'api_presets' },
  { method: 'POST', pattern: '/api/presets', command: 'api_presets_save' },
  { method: 'DELETE', pattern: '/api/presets/{name}', command: 'api_presets_delete' },
  { method: 'POST', pattern: '/api/connect', command: 'api_connect' },
  { method: 'POST', pattern: '/api/disconnect', command: 'api_disconnect' },
  { method: 'POST', pattern: '/api/pair', command: 'api_pair' },
  { method: 'POST', pattern: '/api/tcpip', command: 'api_tcpip' },
  { method: 'POST', pattern: '/api/adb/restart', command: 'api_adb_restart' },
  { method: 'POST', pattern: '/api/scrcpy/launch', command: 'api_scrcpy_launch' },
  { method: 'GET', pattern: '/api/recording/status', command: 'api_recording_status' },
  { method: 'POST', pattern: '/api/recording/start', command: 'api_recording_start' },
  // Остановка ждёт, пока scrcpy допишет файл, — до 45 секунд. Своего таймаута у
  // `invoke` нет, и это как раз то, что нужно: оборванная остановка оставила бы
  // запись без индекса.
  { method: 'POST', pattern: '/api/recording/stop', command: 'api_recording_stop' },
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
      // в команду они должны прийти раскодированными — ровно такими, какими
      // их видит FastAPI в аргументах обработчика.
      params[expected.slice(1, -1)] = decodeURIComponent(actual);
      return true;
    });

    if (matched) return { command: route.command, params };
  }

  return null;
};

/** Тело запроса как JSON. Роутер получает его уже разобранным. */
const parseIpcBody = (body: BodyInit | null | undefined): unknown => {
  // FormData и Blob (загрузка файлов) поедут своей вехой: у них нет
  // осмысленного JSON-представления, а команд под них пока нет.
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

/**
 * Выполняет команду и заворачивает результат в настоящий `Response`.
 *
 * Именно это позволяет не трогать 40+ мест в `App.tsx`, которые работают с
 * результатом `apiFetch` как с ответом `fetch`: `.ok`, `.status`,
 * `readJson()`, `.blob()`, `.headers.get()`.
 */
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
    // которую уже разбирают `readJson()` и `fileErrorMessage()`.
    const failure = error as { status?: unknown; detail?: unknown } | null;
    const status = typeof failure?.status === 'number' ? failure.status : 500;
    const detail = typeof failure?.detail === 'string' ? failure.detail : String(error);
    return jsonResponse({ detail }, status);
  }
};

/**
 * Отдаёт `Response` через IPC, если путь уже переехал, иначе `null` —
 * и вызывающий уходит в `fetch`.
 *
 * Работает только в десктопной сборке: в браузере веб-панель обязана
 * продолжать ходить по HTTP до конца миграции.
 */
const ipcFetch = (path: string, init: ApiFetchOptions): Promise<Response> | null => {
  if (!isTauri()) return null;

  try {
    // Разбор через URL, а не регулярками: нужно отделить `pathname` от
    // `searchParams`, и делать это вручную — верный способ ошибиться на
    // экранировании. База фиктивная, `path` всегда относительный.
    const url = new URL(path, 'http://mkdsc.invalid');
    const method = (init.method || 'GET').toUpperCase();

    const match = matchIpcRoute(method, url.pathname);
    if (!match) return null;

    const query: Record<string, string> = {};
    // Повторяющиеся ключи схлопываются: списочных параметров в API сейчас нет,
    // а когда появятся — здесь понадобится массив.
    url.searchParams.forEach((value, key) => {
      query[key] = value;
    });

    return invokeIpcRoute(match, query, parseIpcBody(init.body));
  } catch {
    // Разбор пути кинуть может: `new URL` на битом пути, `decodeURIComponent`
    // на одиночном '%'. Роутер обязан вести себя как `fetch` до него, поэтому
    // не смогли разобрать — отдаём путь HTTP, а не роняем вызов.
    return null;
  }
};

export const apiFetch = (path: string, init: ApiFetchOptions = {}) => {
  // `timeoutMs` для IPC не нужен — таймаут задаётся на стороне Rust.
  const ipc = ipcFetch(path, init);
  if (ipc) return ipc;

  const { timeoutMs, signal, ...rest } = init;
  if (!timeoutMs) {
    return fetch(apiUrl(path), withAuth(init));
  }

  const controller = new AbortController();
  let cleanupAbort = () => {};
  if (signal) {
    if (signal.aborted) {
      controller.abort();
    } else {
      const onAbort = () => controller.abort();
      signal.addEventListener('abort', onAbort, { once: true });
      cleanupAbort = () => signal.removeEventListener('abort', onAbort);
    }
  }

  const timeoutId = window.setTimeout(() => controller.abort(), timeoutMs);
  return fetch(apiUrl(path), withAuth({ ...rest, signal: controller.signal })).finally(() => {
    window.clearTimeout(timeoutId);
    cleanupAbort();
  });
};

export const readJson = async <T = any>(response: Response): Promise<T> => {
  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return response.json() as Promise<T>;
  }
  const text = await response.text();
  return { error: text || `HTTP ${response.status}` } as T;
};

export const wsUrl = (path: string) => {
  const query = apiToken ? `?token=${encodeURIComponent(apiToken)}` : '';
  if (API_BASE) {
    const wsBase = API_BASE.replace(/^http/, 'ws');
    return `${wsBase}${path}${query}`;
  }
  const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${protocol}://${window.location.host}${path}${query}`;
};

export { API_BASE };
