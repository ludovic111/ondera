import { cssVariables } from "./tokens";

/** Writes every token onto :root as a CSS custom property. Call once before render. */
export function applyTokens(
  root: HTMLElement = document.documentElement,
): void {
  for (const [name, value] of Object.entries(cssVariables())) {
    root.style.setProperty(name, value);
  }
}
