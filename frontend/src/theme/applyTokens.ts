import { cssVariables, setTheme, type Mode } from "./tokens";

export type ModeSetting = Mode | "auto";

/** The one theme's id, written to `data-theme` and to `interface.appearance`. */
export const THEME_ID = "ryolune";

const systemDark = () =>
  typeof matchMedia !== "function" ||
  matchMedia("(prefers-color-scheme: dark)").matches;

export const resolveMode = (mode: ModeSetting | string | undefined): Mode =>
  mode === "light" || mode === "dark" ? mode : systemDark() ? "dark" : "light";

export function applyAppearance(
  mode: Mode,
  root: HTMLElement = document.documentElement,
): void {
  setTheme(mode);
  for (const [name, value] of Object.entries(cssVariables()))
    root.style.setProperty(name, value);
  root.dataset.theme = THEME_ID;
  root.dataset.mode = mode;
  root.style.colorScheme = mode;
  // Canvases repaint from the live token objects.
  window.dispatchEvent(new Event("ryolune:before-capture"));
  window.dispatchEvent(new Event("ryolune:theme"));
}

export function applyTokens(
  root: HTMLElement = document.documentElement,
): void {
  applyAppearance("dark", root);
}
