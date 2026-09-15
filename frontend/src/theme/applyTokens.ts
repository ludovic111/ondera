import { aeroVariables } from "./aero";
import {
  aeroColor,
  aeroFill,
  aeroLine,
  color,
  fill,
  line,
  cssVariables,
} from "./tokens";

const graphite = { ...color };
const original = cssVariables();
const originalFill = { ...fill };
const originalLine = { ...line };
// Replace simultaneously because several graphite tokens share a colour.
const replacements = new Map(
  Object.entries(aeroColor).map(([key, next]) => [
    graphite[key as keyof typeof color] as string,
    next,
  ]),
);
const pattern = [...replacements.keys()]
  .sort((a, b) => b.length - a.length)
  .map((s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
  .join("|");
const replacementPattern = new RegExp(pattern, "g");
export function applyAppearance(
  appearance: "aero" | "graphite",
  root: HTMLElement = document.documentElement,
): void {
  Object.assign(color, graphite, appearance === "aero" ? aeroColor : {});
  Object.assign(fill, originalFill, appearance === "aero" ? aeroFill : {});
  Object.assign(line, originalLine, appearance === "aero" ? aeroLine : {});
  for (const [name, value] of Object.entries(original))
    root.style.setProperty(
      name,
      appearance === "aero"
        ? value.replace(
            replacementPattern,
            (match) => replacements.get(match) ?? match,
          )
        : value,
    );
  // Direct colour variables preserve distinct meanings even when original values match.
  for (const [key, value] of Object.entries(color))
    root.style.setProperty(
      `--color-${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`,
      value,
    );
  for (const [prefix, group] of [
    ["fill", fill],
    ["line", line],
  ] as const)
    for (const [key, value] of Object.entries(group))
      root.style.setProperty(
        `--${prefix}-${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`,
        value,
      );
  if (appearance === "aero")
    for (const [name, value] of Object.entries(aeroVariables))
      root.style.setProperty(name, value);
  root.dataset.appearance = appearance;
  window.dispatchEvent(new Event("ondera:before-capture"));
}
export function applyTokens(
  root: HTMLElement = document.documentElement,
): void {
  applyAppearance("aero", root);
}
