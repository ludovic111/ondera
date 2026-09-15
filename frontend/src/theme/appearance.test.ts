// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { applyAppearance } from "./applyTokens";
import { color, line, cssVariables } from "./tokens";
const graphite = cssVariables();
afterEach(() => applyAppearance("graphite"));
it("restores every original token after switching through Aero repeatedly", () => {
  for (let n = 0; n < 3; n++) {
    applyAppearance("aero");
    expect(document.documentElement.dataset.appearance).toBe("aero");
    expect(color.ink100).toBe("#203b49");
    expect(line.barLine).toContain("28,70,94");
    applyAppearance("graphite");
    for (const [key, value] of Object.entries(graphite))
      expect(document.documentElement.style.getPropertyValue(key), key).toBe(
        value,
      );
  }
});
