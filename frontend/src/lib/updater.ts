import { isTauri } from './api';

export type UpdateInfo = {
  version: string;
  notes: string;
};

export type UpdateProgress = {
  downloaded: number;
  total: number;
  percent: number | null;
};

/**
 * Desktop updates go through tauri-plugin-updater: signed artifacts, an
 * in-place installer swap and a real relaunch.
 *
 * Everything degrades to "open the release page" — in the browser, and in the
 * desktop app whenever the updater is unavailable (no signing key configured
 * yet, endpoint unreachable, portable build).
 */

type PluginUpdate = {
  version: string;
  body?: string | null;
  downloadAndInstall: (
    onEvent?: (event: {
      event: 'Started' | 'Progress' | 'Finished';
      data?: { contentLength?: number; chunkLength?: number };
    }) => void
  ) => Promise<void>;
};

let pendingUpdate: PluginUpdate | null = null;

export const canSelfUpdate = () => isTauri();

/**
 * Returns the available update, `null` when already current, or throws when
 * the updater itself is not usable — callers fall back to the release page.
 */
export const checkForUpdate = async (): Promise<UpdateInfo | null> => {
  const { check } = await import('@tauri-apps/plugin-updater');
  const update = (await check()) as PluginUpdate | null;
  pendingUpdate = update;
  if (!update) return null;
  return { version: update.version, notes: (update.body || '').trim() };
};

export const installUpdate = async (onProgress?: (progress: UpdateProgress) => void) => {
  if (!pendingUpdate) {
    throw new Error('No update has been checked for');
  }
  let downloaded = 0;
  let total = 0;
  await pendingUpdate.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      total = event.data?.contentLength || 0;
      downloaded = 0;
    } else if (event.event === 'Progress') {
      downloaded += event.data?.chunkLength || 0;
    } else if (event.event === 'Finished') {
      downloaded = total;
    }
    onProgress?.({
      downloaded,
      total,
      percent: total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null
    });
  });
  pendingUpdate = null;
};

export const restartApp = async () => {
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
};

export const openReleasePage = async (url: string) => {
  if (!url) return;
  window.open(url, '_blank', 'noopener,noreferrer');
};
