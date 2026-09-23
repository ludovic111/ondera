import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The groove's step dots and the agent's tool steps shared the class `.steps` in one CSS
// module, so `.steps span { width: 12px }` squeezed every tool-step label to one word a line.
describe("AgentPanel.module.css", () => {
  const css = readFileSync(
    new URL("./AgentPanel.module.css", import.meta.url),
    "utf8",
  );
  it("sizes only the groove's dots, not the tool steps' labels", () => {
    expect(css).not.toMatch(/\.steps\s+span/);
    expect(css).toMatch(/\.grooveSteps\s+span\s*\{[^}]*width:\s*12px/);
  });
});
