import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * Откуда браузеру брать картинку скриншота.
 *
 * URL у файла на диске нет, поэтому используется asset-протокол Tauri. Путь
 * приходит в поле `path` ответа `GET /api/screenshots`.
 *
 * data-URL вместо этого не годится: скриншот весит мегабайты, и страница
 * галереи из двадцати плиток раздулась бы на десятки мегабайт разметки.
 */
export const screenshotSrc = (screenshot: { path?: string }) =>
  screenshot.path ? convertFileSrc(screenshot.path) : '';
