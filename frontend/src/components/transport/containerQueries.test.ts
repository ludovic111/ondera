import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

// `.bar` is the query container, so `@container { .bar { … } }` never applied: the narrow
// transport kept its full spacing and the master meter clipped at the window's minimum width.
it("the transport's container queries style what is inside the bar, not the bar", () => {
  const css = readFileSync(
    new URL("./TransportBar.module.css", import.meta.url),
    "utf8",
  );
  const blocks = css.split("@container").slice(1);
  expect(blocks.length).toBeGreaterThan(0);
  for (const block of blocks) {
    const body = block.slice(block.indexOf("{") + 1);
    expect(body).not.toMatch(/(^|[\s,}])\.bar\s*[{,]/);
  }
  expect(css).toMatch(/max-width: 1200px[^}]*\.meter > :first-child/);
});
