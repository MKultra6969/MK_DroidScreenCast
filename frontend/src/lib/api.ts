const envBase = (import.meta.env.VITE_API_BASE || '').replace(/\/$/, '');
const API_BASE = envBase || '';

export const apiUrl = (path: string) => `${API_BASE}${path}`;
type ApiFetchOptions = RequestInit & { timeoutMs?: number };

export const apiFetch = (path: string, init: ApiFetchOptions = {}) => {
  const { timeoutMs, signal, ...rest } = init;
  if (!timeoutMs) {
    return fetch(apiUrl(path), init);
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
  return fetch(apiUrl(path), { ...rest, signal: controller.signal }).finally(() => {
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
  if (API_BASE) {
    const wsBase = API_BASE.replace(/^http/, 'ws');
    return `${wsBase}${path}`;
  }
  const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${protocol}://${window.location.host}${path}`;
};

export { API_BASE };
