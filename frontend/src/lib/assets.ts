import { convertFileSrc } from '@tauri-apps/api/core';

import { apiUrlWithToken, isTauri } from './api';

/**
 * Откуда браузеру брать картинку скриншота.
 *
 * В вебе это HTTP-эндпоинт с токеном в адресе: заголовки к `<img src>` не
 * прицепить. В десктопе URL не существует вовсе — там файл читается прямо с
 * диска по asset-протоколу Tauri. Путь приходит в поле `path` ответа
 * `GET /api/screenshots`, которое добавляет Rust; Python его не отдаёт,
 * поэтому запасной вариант остаётся рабочим и в смешанном режиме.
 *
 * data-URL вместо этого не годится: скриншот весит мегабайты, и страница
 * галереи из двадцати плиток раздулась бы на десятки мегабайт разметки.
 */
export const screenshotSrc = (screenshot: { id: string; path?: string }) => {
  if (isTauri() && screenshot.path) {
    return convertFileSrc(screenshot.path);
  }
  return apiUrlWithToken(`/api/screenshots/${screenshot.id}`);
};
