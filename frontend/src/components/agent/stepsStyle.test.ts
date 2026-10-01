import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The Rhythm Lab's step dots and the agent's tool steps once shared the class `.steps` in one
// CSS module, so `.steps span { width: 12px }` squeezed every tool-step label to one word a
// line. Nothing may size the tool steps' labels like dots again.
describe("AgentPanel.module.css", () => {
  const css = readFileSync(
    new URL("./AgentPanel.module.css", import.meta.url),
    "utf8",
  );
  it("never sizes the tool steps' labels", () => {
    expect(css).not.toMatch(/\.steps\s+span/);
  });
});
