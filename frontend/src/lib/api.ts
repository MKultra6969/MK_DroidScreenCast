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

export const apiFetch = (path: string, init: ApiFetchOptions = {}) => {
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
