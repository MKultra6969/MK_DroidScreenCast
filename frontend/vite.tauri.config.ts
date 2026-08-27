import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// Единственная сборка приложения. Страница открывается WebView по file://,
// поэтому пути к ресурсам относительные; в бэкенд фронтенд ходит не по сети, а
// через IPC (см. `src/lib/api.ts`), так что ни адреса, ни прокси здесь нет.
export default defineConfig({
  base: './',
  plugins: [react()],
  build: {
    outDir: resolve(__dirname, 'dist-tauri'),
    assetsDir: 'assets',
    emptyOutDir: true,
    // Страницу показывает WebView конкретной платформы, а не «браузер вообще»:
    // на Windows это Chromium из WebView2, на macOS — WKWebView. Сборка под
    // современный движок избавляет от полифилов и транспиляции, которые здесь
    // всё равно никому не нужны.
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome110' : 'safari15',
    // Карты кода утраивают вес каталога сборки, а читать их в готовом
    // приложении некому.
    sourcemap: false,
    // Считать gzip-размер незачем: файлы отдаются с диска, а не по сети.
    reportCompressedSize: false,
    rollupOptions: {
      output: {
        // React меняется куда реже кода приложения. Отдельным файлом он
        // остаётся в кэше WebView после обновления приложения, а не
        // перечитывается заново вместе с ним.
        manualChunks: {
          react: ['react', 'react-dom']
        }
      }
    }
  },
  esbuild: {
    // console.* оставляем: приложение пишет туда диагностику, которую просят
    // приложить к отчётам об ошибках.
    drop: ['debugger']
  },
  server: {
    port: 5173,
    strictPort: true
  }
});
