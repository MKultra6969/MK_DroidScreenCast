import { isTauri } from './api';

/**
 * `window.confirm` is unreliable inside a WebView (WebKitGTK often returns
 * false outright), so the desktop build uses the Tauri dialog plugin that is
 * already a dependency.
 */
export const confirmAction = async (message: string, title = 'MK DroidScreenCast') => {
  if (isTauri()) {
    try {
      const { ask } = await import('@tauri-apps/plugin-dialog');
      return await ask(message, { title, kind: 'warning' });
    } catch (error) {
      console.error('confirm dialog error', error);
    }
  }
  return window.confirm(message);
};
