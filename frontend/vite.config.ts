import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

export default defineConfig({
  base: '/static/',
  plugins: [react()],
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
  },
  build: {
    outDir: resolve(__dirname, '..', 'static'),
    assetsDir: 'assets',
    emptyOutDir: true
  }
});
