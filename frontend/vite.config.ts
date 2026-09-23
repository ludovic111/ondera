import { defineConfig } from "vitest/config";
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
  // Shared CI runners (the Intel macOS one above all) can take several seconds for the
  // theme audits that rebuild every variant; the default 5 s failed a release run.
  test: { testTimeout: 30_000 },
});
