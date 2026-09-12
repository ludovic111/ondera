import { defineConfig } from 'electron-vite';
import react from '@vitejs/plugin-react';

// electron-vite resolves out/main/index.js and out/preload/index.js by name,
// so the bundles are renamed regardless of the entry file names.
export default defineConfig({
  main: {
    build: {
      lib: { entry: 'electron/main.ts' },
      rollupOptions: { output: { entryFileNames: 'index.js' } },
    },
  },
  preload: {
    build: {
      lib: { entry: 'electron/preload.ts' },
      rollupOptions: { output: { entryFileNames: 'index.js' } },
    },
  },
  renderer: {
    root: 'src',
    build: {
      // Relative to the renderer root above.
      rollupOptions: { input: 'index.html' },
    },
    plugins: [react()],
  },
});
