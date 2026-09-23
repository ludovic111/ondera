import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

// The Tauri webview (wry on macOS) implements no JavaScript text-input panel: window.prompt
// returns null at once, so "Move to new folder…" did nothing. The same holds for confirm and
// alert. Ask in the page instead.
it("the interface never relies on window.prompt, confirm or alert", () => {
  const root = fileURLToPath(new URL("../..", import.meta.url));
  const offenders: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)) {
        const text = readFileSync(path, "utf8");
        if (/\bwindow\.(prompt|confirm|alert)\s*\(/.test(text))
          offenders.push(path);
      }
    }
  };
  walk(root);
  expect(offenders).toEqual([]);
});
