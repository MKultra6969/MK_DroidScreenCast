/*
 * Тема — до первой отрисовки.
 *
 * React монтируется через несколько сотен миллисекунд после разбора
 * документа, и всё это время пользователь тёмной темы смотрел на белый экран.
 * Ключ хранилища тот же, что читает initTheme() в App.tsx.
 *
 * Отдельным файлом, а не строчкой в index.html: CSP приложения разрешает
 * только `script-src 'self'` — встроенный <script> движок просто не выполнит.
 */
(function () {
  try {
    var pref = localStorage.getItem('themePreference') || 'auto';
    var dark =
      pref === 'dark' ||
      (pref === 'auto' &&
        window.matchMedia &&
        window.matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  } catch (error) {
    /* приватный режим или заблокированное хранилище — остаётся светлая тема */
  }
})();
