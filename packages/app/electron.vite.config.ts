import { defineConfig } from 'electron-vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  main: {
    build: {
      lib: { entry: 'electron/main.ts' },
    },
  },
  preload: {
    build: {
      lib: { entry: 'electron/preload.ts' },
    },
  },
  renderer: {
    root: 'src',
    build: {
      rollupOptions: { input: 'src/index.html' },
    },
    plugins: [react()],
  },
});
