import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

// CLAUDE.md: frontend/src/theme is the only place visual values live. ThemePicker carried
// literal oklch() track colours copied from the palette.
it("components, canvas code and state hold no literal colours", () => {
  const src = fileURLToPath(new URL("..", import.meta.url));
  const found: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (/\.(tsx?|css)$/.test(name) && !/\.test\.tsx?$/.test(name)) {
        const text = readFileSync(path, "utf8");
        if (/oklch\(\s*[\d.]|#[0-9a-fA-F]{6}\b|rgba?\(\s*\d/.test(text))
          found.push(path);
      }
    }
  };
  for (const dir of ["components", "canvas", "state"]) walk(join(src, dir));
  expect(found).toEqual([]);
});
