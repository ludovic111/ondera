import { cssVariables, setTheme, type Mode, type ThemeId } from "./tokens";
import { THEMES } from "./schema";

export type ModeSetting = Mode | "auto";

/** Settings written before 0.6 named the two appearances differently. */
export function normalizeTheme(value: string | undefined): ThemeId {
  if (value === "graphite") return "skeuo";
  return (THEMES as readonly string[]).includes(value ?? "")
    ? (value as ThemeId)
    : "skeuo";
}

const systemDark = () =>
  typeof matchMedia !== "function" ||
  matchMedia("(prefers-color-scheme: dark)").matches;

export const resolveMode = (mode: ModeSetting | undefined): Mode =>
  mode === "light" || mode === "dark" ? mode : systemDark() ? "dark" : "light";

let emitted: string[] = [];

export function applyAppearance(
  theme: ThemeId,
  mode: Mode,
  root: HTMLElement = document.documentElement,
): void {
  setTheme(theme, mode);
  const vars = cssVariables();
  // Theme-only properties of the previous theme must not leak into this one.
  for (const name of emitted)
    if (!(name in vars)) root.style.removeProperty(name);
  for (const [name, value] of Object.entries(vars))
    root.style.setProperty(name, value);
  emitted = Object.keys(vars);
  root.dataset.theme = theme;
  root.dataset.mode = mode;
  root.style.colorScheme = mode;
  // Canvases repaint from the live token objects.
  window.dispatchEvent(new Event("ondera:before-capture"));
  window.dispatchEvent(new Event("ondera:theme"));
}

export function applyTokens(
  root: HTMLElement = document.documentElement,
): void {
  applyAppearance("skeuo", "dark", root);
}
