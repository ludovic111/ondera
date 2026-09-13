import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@ondera/core": fileURLToPath(
        new URL("./src/core/index.ts", import.meta.url),
      ),
    },
  },
  clearScreen: false,
  build: { target: "es2022" },
});
