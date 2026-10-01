// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { applyAppearance, THEME_ID } from "./applyTokens";
import { buildTheme, color, cssVariables, font, line } from "./tokens";
import { contrast, FAMILY_HUES, type Mode } from "./schema";
import { mix, parseColor } from "./color";

const MODES: Mode[] = ["dark", "light"];

afterEach(() => applyAppearance("dark"));

it("restores every token after switching modes repeatedly", () => {
  applyAppearance("dark");
  const original = cssVariables();
  for (let n = 0; n < 2; n++) {
    for (const mode of MODES) {
      applyAppearance(mode);
      expect(document.documentElement.dataset.theme).toBe(THEME_ID);
      expect(document.documentElement.dataset.mode).toBe(mode);
      expect(color.ink100).toBe(buildTheme(mode).color.ink100);
      expect(line.barLine).toBe(buildTheme(mode).line.barLine);
    }
    applyAppearance("dark");
    expect(font.ui).toContain("Manrope");
    const style = document.documentElement.style;
    for (const [key, value] of Object.entries(original))
      expect(style.getPropertyValue(key), key).toBe(value);
  }
});

it("gives both modes the same custom properties", () => {
  const keys = (mode: Mode) => Object.keys(buildTheme(mode).vars).sort();
  expect(keys("light")).toEqual(keys("dark"));
});

describe.each(MODES)("ryolune %s", (mode) => {
  const spec = buildTheme(mode);
  const c = spec.color;

  it("emits only colours the canvas can parse", () => {
    for (const group of [spec.color, spec.fill, spec.line])
      for (const [key, value] of Object.entries(group))
        if (value !== "transparent")
          expect(() => parseColor(value), key).not.toThrow();
  });

  it("keeps text legible on every reading surface", () => {
    const surfaces = [c.panel, c.timeline, c.editor, c.agentPanel, c.menu];
    for (const surface of surfaces) {
      expect(contrast(c.ink100, surface)).toBeGreaterThanOrEqual(7);
      expect(contrast(c.ink300, surface)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(c.ink500, surface)).toBeGreaterThanOrEqual(3.6);
    }
    expect(contrast(c.inkControl, c.controlBottom)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(c.wellInk, c.wellDeep)).toBeGreaterThanOrEqual(7);
    expect(contrast(c.wellInkFaint, c.wellDeep)).toBeGreaterThanOrEqual(3.6);
  });

  it("keeps markers visible on the ruler and their names readable", () => {
    expect(contrast(spec.line.marker, c.ruler)).toBeGreaterThanOrEqual(3);
    const flag = parseColor(spec.fill.markerFlag);
    const chip = mix(
      `rgb(${flag.r * 255},${flag.g * 255},${flag.b * 255})`,
      c.ruler,
      flag.a * 100,
    );
    expect(contrast(c.inkBright, chip)).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps the accent visible and its label readable", () => {
    expect(contrast(c.accent, c.panel)).toBeGreaterThanOrEqual(3);
    expect(contrast(c.accentInk, c.accentLo)).toBeGreaterThanOrEqual(3);
  });

  it("keeps every sound-family colour visible on panels and in display wells", () => {
    for (const key of Object.keys(
      FAMILY_HUES,
    ) as (keyof typeof FAMILY_HUES)[]) {
      expect(contrast(c[key], c.panel), key).toBeGreaterThanOrEqual(3);
      expect(contrast(c[key], c.menu), key).toBeGreaterThanOrEqual(3);
    }
  });

  it("keeps clip names readable on the darkest and lightest track colours", () => {
    for (const track of ["oklch(0.72 0.14 40)", "oklch(0.78 0.14 85)"]) {
      const face = mix(track, c.panel, spec.clipMix.faceBottom);
      const name = parseColor(spec.line.clipName);
      // The name is drawn at its own alpha over the face.
      const flat = mix(
        `rgb(${name.r * 255},${name.g * 255},${name.b * 255})`,
        face,
        name.a * 100,
      );
      expect(contrast(flat, face)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("orders the surface ladder the same way in both modes", () => {
    const L = (s: string) => {
      const { r, g, b } = parseColor(s);
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    // Controls always stand proud of the panel they sit on.
    expect(L(c.controlTop)).toBeGreaterThan(L(c.panel));
    // Pressed is always darker than raised.
    expect(L(c.pressedTop)).toBeLessThan(L(c.controlTop));
  });
});
