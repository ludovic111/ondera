/**
 * THE tokens file. Every colour, font size, radius, dimension, gradient and
 * shadow in the app comes from here, either directly (canvas code) or through
 * the CSS custom properties emitted by applyAppearance().
 *
 * Scales that never change (type, spacing, dimensions) are constants below.
 * Everything visual is themed: `color`, `gradient`, `shadow`, `fill`, `line`,
 * `blur`, `radius`, `canvasShadow` and `clipMix` are live objects that
 * setTheme() refills from one of the three themes (modern.ts, skeuo.ts,
 * aero.ts), each in a dark and a light mode. Canvas code reads them at paint
 * time, so it follows the theme without subscribing to anything.
 *
 * Do not add visual constants anywhere else.
 */
import { aero } from "./aero";
import { modern } from "./modern";
import { skeuo } from "./skeuo";
import type { Mode, ThemeId, ThemeSpec } from "./schema";

export { white, black } from "./schema";
export type { CanvasShadowLayer, Mode, ThemeId } from "./schema";

const BUILDERS: Record<ThemeId, (mode: Mode) => ThemeSpec> = {
  modern,
  skeuo,
  aero,
};

export function buildTheme(theme: ThemeId, mode: Mode): ThemeSpec {
  return BUILDERS[theme](mode);
}

// Live groups. Skeuomorphic dark is the design source, so it is the default.
const initial = buildTheme("skeuo", "dark");
export const color = { ...initial.color };
export const gradient = { ...initial.gradient };
export const shadow = { ...initial.shadow };
export const fill = { ...initial.fill };
export const line = { ...initial.line };
export const blur = { ...initial.blur };
export const radius = { ...initial.radius };
export const motion = { ...initial.motion };
export const canvasShadow = { ...initial.canvasShadow };
export const clipMix = { ...initial.clipMix };
let themeVars: Record<string, string> = { ...initial.vars };
let scheme: Mode = initial.scheme;

// ---------------------------------------------------------------------------
// Type
// ---------------------------------------------------------------------------

export const font: { ui: string; mono: string } = {
  ui: "'Manrope', system-ui, -apple-system, sans-serif",
  mono: "'IBM Plex Mono', ui-monospace, Menlo, monospace",
};

/** Sizes in px. Names follow the spec sheet's type scale. */
export const fontSize = {
  transport: 19,
  panelTitle: 14,
  prose: 13,
  agentInput: 12.5,
  body: 12,
  list: 11.5,
  secondary: 11,
  value: 10.5,
  small: 10,
  caps: 9.5,
  kind: 9,
  micro: 8.5,
} as const;

export const fontWeight = {
  regular: 400,
  medium: 500,
  semibold: 600,
  bold: 700,
} as const;

export const tracking: { caps: string; capsWide: string; kind: string } = {
  caps: "0.09em",
  capsWide: "0.1em",
  kind: "0.05em",
};

export const lineHeight = {
  digits: 1.05,
  ui: 1.3,
  prose: 1.45,
  block: 1.5,
} as const;

// ---------------------------------------------------------------------------
// Grid, radii, dimensions
// ---------------------------------------------------------------------------

export const space = {
  unit: 4,
  xs: 4,
  sm: 6,
  md: 8,
  lg: 10,
  xl: 12,
  xxl: 14,
  xxxl: 16,
} as const;

export const size = {
  windowWidth: 1600,
  windowHeight: 1000,
  minWindowWidth: 1280,
  minWindowHeight: 800,
  titleBar: 28,
  transport: 52,
  arrangeToolbar: 32,
  ruler: 28,
  trackRow: 70,
  editor: 300,
  editorHeader: 32,
  editorRuler: 18,
  keyColumn: 56,
  keyRow: 10,
  keyRows: 25,
  /** Height of the controller lane under the piano roll. */
  controllerLane: 96,
  /** Radius of a controller point, and how near the pointer must come to grab one. */
  controllerPoint: 3,
  controllerGrip: 6,
  /** Space above the highest and below the lowest controller value. */
  controllerInset: 6,
  agentHeader: 44,
  browser: 220,
  trackHeader: 184,
  inspector: 240,
  agentPanel: 380,
  agentRail: 32,
  buttonW: 34,
  buttonH: 26,
  playButtonW: 44,
  smallButtonW: 20,
  smallButtonH: 17,
  timeDisplayH: 34,
  knobSm: 20,
  knobMd: 26,
  knobLg: 36,
  knobXl: 46,
  sliderThumb: 13,
  sliderRailH: 4,
  sliderTravel: 60,
  faderH: 150,
  faderRailW: 8,
  faderCapW: 28,
  faderCapH: 18,
  ledSegW: 5,
  ledSegH: 6,
  ledCpuH: 14,
  zoomRailW: 90,
  zoomThumb: 12,
  trafficLight: 11,
  trafficLightInset: 78,
  colorStrip: 5,
  clipInset: 5,
  clipTitle: 14,
  clipNoteH: 3,
  /** Grab zone at each clip edge for trimming, in px. */
  clipEdgeGrip: 7,
  /** Grab zone at each cycle-range edge in the ruler, in px. */
  cycleGrip: 6,
  /** Fade handle at an audio clip's top corners, and its grab zone, in px. */
  fadeHandle: 7,
  fadeGrip: 6,
  /** Marker flag in the ruler: its top and height, in px. */
  markerTop: 15,
  markerH: 12,
  /** Grab zone at a note's right edge for resizing, in px. */
  noteEdgeGrip: 5,
  menuMinW: 200,
  menuItemH: 24,
  menuPad: 4,
  inlineInputH: 18,
} as const;

export const timeline = {
  pxPerBar: 48,
  editorPxPerBar: 88,
  /** Long clips shrink in the editor down to this before they get cut off. */
  editorMinPxPerBar: 28,
  staffLineGap: 8,
  noteHeadR: 3.5,
  minPxPerBar: 12,
  maxPxPerBar: 480,
  rulerTickH: 6,
  playheadFlagW: 14,
  playheadFlagH: 8,
} as const;

/** Refill the live groups. Returns the spec so the caller can emit CSS. */
export function setTheme(theme: ThemeId, mode: Mode): ThemeSpec {
  const spec = buildTheme(theme, mode);
  Object.assign(color, spec.color);
  Object.assign(gradient, spec.gradient);
  Object.assign(shadow, spec.shadow);
  Object.assign(fill, spec.fill);
  Object.assign(line, spec.line);
  Object.assign(blur, spec.blur);
  Object.assign(radius, spec.radius);
  Object.assign(motion, spec.motion);
  Object.assign(canvasShadow, spec.canvasShadow);
  Object.assign(clipMix, spec.clipMix);
  font.ui = spec.fontUi;
  tracking.caps = spec.capsTracking;
  themeVars = { ...spec.vars };
  scheme = spec.scheme;
  return spec;
}

export const colorScheme = (): Mode => scheme;

// ---------------------------------------------------------------------------
// CSS custom properties
// ---------------------------------------------------------------------------

const kebab = (s: string) =>
  s.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

function emit(
  vars: Record<string, string>,
  prefix: string,
  group: Record<string, string | number>,
  unit = "",
) {
  for (const [k, v] of Object.entries(group)) {
    vars[`--${prefix}-${kebab(k)}`] = typeof v === "number" ? `${v}${unit}` : v;
  }
}

/** Every token of the current theme as a CSS custom property. */
export function cssVariables(): Record<string, string> {
  const vars: Record<string, string> = {};
  emit(vars, "color", color);
  emit(vars, "font", font);
  emit(vars, "fs", fontSize, "px");
  emit(vars, "fw", fontWeight);
  emit(vars, "tracking", tracking);
  emit(vars, "lh", lineHeight);
  emit(vars, "space", space, "px");
  emit(vars, "radius", radius, "px");
  emit(vars, "size", size, "px");
  emit(vars, "timeline", timeline, "px");
  emit(vars, "gradient", gradient);
  emit(vars, "shadow", shadow);
  emit(vars, "fill", fill);
  emit(vars, "line", line);
  emit(vars, "blur", blur);
  emit(vars, "motion", motion);
  return { ...vars, ...themeVars };
}
