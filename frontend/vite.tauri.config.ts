import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

const DEFAULT_API_BASE = 'http://127.0.0.1:6969';

export default defineConfig(({ mode }) => {
  // В десктопной сборке WebView открывает страницу по file://, поэтому
  // относительный `/api` никуда не ведёт — фронту нужен абсолютный адрес
  // бэкенда из VITE_API_BASE.
  //
  // Значение лежит в `frontend/.env.tauri` (файл в репозитории). Раньше он был
  // только на машине разработчика: из чистого клона сборка выходила с пустым
  // VITE_API_BASE, все запросы уходили в origin WebView и приложение вечно
  // висело на экране загрузки. Дефолт ниже делает отсутствие файла
  // некритичным, но своё значение из .env.tauri по-прежнему выигрывает —
  // иначе нельзя было бы переехать на другой порт.
  const env = loadEnv(mode, __dirname, 'VITE_');
  const apiBase = env.VITE_API_BASE || DEFAULT_API_BASE;

  return {
    base: './',
    plugins: [react()],
    define: {
      'import.meta.env.VITE_API_BASE': JSON.stringify(apiBase)
    },
    build: {
      outDir: resolve(__dirname, 'dist-tauri'),
      assetsDir: 'assets',
      emptyOutDir: true
    },
    server: {
      port: 5173,
      strictPort: true,
      proxy: {
        '/api': {
          target: DEFAULT_API_BASE,
          changeOrigin: true
        },
        '/ws': {
          target: 'ws://127.0.0.1:6969',
          ws: true,
          changeOrigin: true
        }
      }
    }
  };
});
