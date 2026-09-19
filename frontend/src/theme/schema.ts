/**
 * Shape of a theme. A theme is a pure function of the mode that returns every
 * themed token group; tokens.ts holds the live copies the app reads.
 */
import { formatRgba, parseColor } from "./color";

export type ThemeId = "modern" | "skeuo" | "aero";
export type Mode = "dark" | "light";
export const THEMES: readonly ThemeId[] = ["modern", "skeuo", "aero"];
export const THEME_NAMES: Record<ThemeId, string> = {
  modern: "Modern",
  skeuo: "Skeuomorphic",
  aero: "Frutiger Aero",
};

export type ColorKey =
  | "desk"
  | "wellDeep"
  | "groove"
  | "grooveAlt"
  | "timelineEmpty"
  | "timeline"
  | "timelineAgent"
  | "timelineSelected"
  | "editor"
  | "ruler"
  | "materialCard"
  | "agentPanel"
  | "panel"
  | "logEntry"
  | "raised"
  | "controlFace"
  | "controlTop"
  | "controlBottom"
  | "pressedTop"
  | "pressedBottom"
  | "transportTop"
  | "transportBottom"
  | "segmentTop"
  | "segmentBottom"
  | "thumbTop"
  | "thumbBottom"
  | "knobHi"
  | "knobLo"
  | "knobBigHi"
  | "knobBigLo"
  | "knobInnerHi"
  | "knobInnerLo"
  | "capTop"
  | "capMid"
  | "capBottom"
  | "headerTop"
  | "headerBottom"
  | "headerSelectedTop"
  | "headerSelectedBottom"
  | "headerAgentTop"
  | "headerAgentBottom"
  | "insertTop"
  | "insertBottom"
  | "chipTop"
  | "chipBottom"
  | "trafficLight"
  | "inkBright"
  | "ink100"
  | "inkControl"
  | "inkDim"
  | "ink300"
  | "ink500"
  | "inkSeparator"
  | "inkKeyWhite"
  | "wellInk"
  | "wellInkDim"
  | "wellInkFaint"
  | "keyWhite"
  | "keyBlack"
  | "led"
  | "ledOff"
  | "ledGlow"
  | "ledGlowSoft"
  | "ledHot"
  | "accent"
  | "accentHi"
  | "accentLo"
  | "accentInk"
  | "noteTop"
  | "noteBottom"
  | "eqCurve"
  | "eqHandle"
  | "eqFill"
  | "neutralDot"
  | "menu"
  | "menuHover"
  | "menuSeparator"
  | "indicator"
  | "danger"
  | "scrim";

export type GradientKey =
  | "raised"
  | "raisedHover"
  | "pressed"
  | "lit"
  | "transport"
  | "titleBar"
  | "segment"
  | "thumb"
  | "knob"
  | "knobBig"
  | "knobInner"
  | "faderCap"
  | "header"
  | "headerSelected"
  | "headerAgent"
  | "panelHeader"
  | "insert"
  | "chip"
  | "sendKey"
  | "note"
  | "eqGrid"
  | "dialog"
  | "dialogHeader"
  | "plate";

export type ShadowKey =
  | "raised"
  | "raisedSm"
  | "pressed"
  | "lit"
  | "knob"
  | "knobBig"
  | "knobInner"
  | "knobIndicator"
  | "faderCap"
  | "thumb"
  | "groove"
  | "grooveSoft"
  | "grooveShallow"
  | "grooveSend"
  | "wellDeep"
  | "wellInput"
  | "wellValue"
  | "segment"
  | "clip"
  | "clipAgent"
  | "clipSelected"
  | "ledLit"
  | "ledAccent"
  | "ledOff"
  | "ledEmpty"
  | "glass"
  | "glassAgent"
  | "glassCard"
  | "accentDot"
  | "accentDotLg"
  | "accentBar"
  | "playhead"
  | "playheadRuler"
  | "headerCell"
  | "headerCellAgent"
  | "headerColumn"
  | "colorStrip"
  | "swatch"
  | "swatchSm"
  | "trafficLight"
  | "titleBar"
  | "transport"
  | "toolbar"
  | "toolbarEditor"
  | "rulerCorner"
  | "panelLeft"
  | "panelRight"
  | "panelAgent"
  | "panelAgentRail"
  | "editorPane"
  | "keyColumn"
  | "keyRow"
  | "divider"
  | "dividerTop"
  | "logEntry"
  | "logChip"
  | "insert"
  | "insertEmpty"
  | "ledInsertOff"
  | "window"
  | "sendKey"
  | "menu"
  | "dialog"
  | "inlineInput"
  | "note"
  | "faderLineHi"
  | "milledHi"
  | "plate"
  | "focus";

export type FillKey =
  | "glass"
  | "glassAgent"
  | "glassCard"
  | "logLive"
  | "logChipLive"
  | "browserHighlight"
  | "cycleRuler"
  | "cycleLane"
  | "clipTitle"
  | "rowShade"
  | "blackKeyRow"
  | "eqGridLine"
  | "eqZeroLine"
  | "velocity"
  | "dragGhost"
  | "pencilPreview"
  | "cycleHandle"
  | "dropTarget"
  | "stepCell"
  | "hover";

export type LineKey =
  | "barLine"
  | "beatLine"
  | "rulerBar"
  | "rulerTick"
  | "cycleEdge"
  | "editorBar"
  | "editorBeat"
  | "waveform"
  | "waveformMid"
  | "midiNote"
  | "clipName"
  | "clipTitleBottom"
  | "clipHighlight"
  | "clipContact"
  | "clipSelected"
  | "cellDivider"
  | "dividerDark"
  | "laneTop"
  | "laneBottom"
  | "rulerBottom"
  | "faderLine"
  | "faderLineHi"
  | "milled"
  | "milledHi"
  | "dragGhostEdge"
  | "noteSelected"
  | "noteHighlight"
  | "splitGuide"
  | "staff"
  | "noteHead"
  | "border"
  | "borderStrong"
  | "hairline";

export type BlurKey = "glass" | "glassAgent" | "glassCard";
export type RadiusKey =
  | "xs"
  | "sm"
  | "clip"
  | "button"
  | "md"
  | "control"
  | "lg"
  | "glass"
  | "window";

export interface CanvasShadowLayer {
  blur: number;
  offsetY: number;
  color: string;
}
export type CanvasShadowKey =
  | "clipDrop"
  | "playhead"
  | "playheadRuler"
  | "playheadFlag"
  | "bubble"
  | "agentRing"
  | "agentNote"
  | "noteSelected"
  | "note";

export interface ThemeSpec {
  scheme: Mode;
  color: Record<ColorKey, string>;
  gradient: Record<GradientKey, string>;
  shadow: Record<ShadowKey, string>;
  fill: Record<FillKey, string>;
  line: Record<LineKey, string>;
  blur: Record<BlurKey, string>;
  radius: Record<RadiusKey, number>;
  canvasShadow: Record<CanvasShadowKey, readonly CanvasShadowLayer[]>;
  /** Percent of track colour mixed against the panel for clip faces. */
  clipMix: { faceTop: number; faceBottom: number };
  fontUi: string;
  /** Tracking of caps labels; modern and aero set type less mechanically. */
  capsTracking: string;
  /** Theme-only custom properties consumed by the theme's own stylesheet. */
  vars: Record<string, string>;
}

// ---------------------------------------------------------------------------
// Colour helpers. Everything resolves to rgba() so canvas code can use it.
// ---------------------------------------------------------------------------

/** oklch(L C H) as an sRGB rgba() string. */
export const ok = (L: number, C: number, H: number, a = 1): string =>
  formatRgba(parseColor(`oklch(${L} ${C} ${H})`), a);

export const alpha = (colour: string, a: number): string =>
  formatRgba(parseColor(colour), a);

export const white = (a: number) => `rgba(255,255,255,${a})`;
export const black = (a: number) => `rgba(0,0,0,${a})`;

/** WCAG relative luminance contrast between two opaque colours. */
export function contrast(a: string, b: string): number {
  const lum = (s: string) => {
    const { r, g, b } = parseColor(s);
    const f = (c: number) =>
      c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
