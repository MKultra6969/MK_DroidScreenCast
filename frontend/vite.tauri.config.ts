import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

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
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:6969',
        changeOrigin: true
      },
      '/ws': {
        target: 'ws://127.0.0.1:6969',
        ws: true,
        changeOrigin: true
      }
    }
  }
});
