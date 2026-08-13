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
    emptyOutDir: true
  },
  server: {
    port: 5173,
    strictPort: true
  }
});
