/**
 * THE tokens file. Every colour, font size, radius, dimension, gradient and
 * shadow in the app comes from here, either directly (canvas code, Electron
 * main) or through the CSS custom properties emitted by applyTokens().
 *
 * Source: design/Ondera Arrangement.dc.html, spec sheet 02 (skeuomorphic
 * material system, light from 90° above). Do not add visual constants
 * anywhere else.
 */

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

export const color = {
  // Graphite scale: oklch hue 90, chroma 0.003. Warm-neutral, never blue.
  desk: "#141413", // graphite-950
  wellDeep: "#161615", // time display, meters, EQ well
  groove: "#1a1a19", // graphite-900: grooves, wells, slider rails
  grooveAlt: "#1c1c1b", // segmented-control and search grooves
  timelineEmpty: "#1f1f1e", // below the last track, piano roll grid
  timeline: "#222221", // graphite-800: lanes
  timelineAgent: "#222524", // lane of a track an agent is editing
  timelineSelected: "#262626", // lane of the selected track
  editor: "#262625", // piano roll pane, header column below tracks
  ruler: "#272726", // bar ruler background
  materialCard: "#282827",
  agentPanel: "#29292a",
  panel: "#2c2c2b", // graphite-700: side panels, title bar
  logEntry: "#2f2f2e",
  raised: "#353534", // graphite-600
  controlFace: "#42423f", // graphite-500

  // Gradient stops for the material recipes.
  controlTop: "#42423f",
  controlBottom: "#313130",
  pressedTop: "#262625",
  pressedBottom: "#2c2c2b",
  transportTop: "#333332",
  transportBottom: "#2c2c2b",
  segmentTop: "#45453f",
  segmentBottom: "#363634",
  thumbTop: "#5a5a57",
  thumbBottom: "#3a3a38",
  knobHi: "#4c4c49",
  knobLo: "#2a2a29",
  knobBigHi: "#555552",
  knobBigLo: "#2c2c2b",
  knobInnerHi: "#3a3a38",
  knobInnerLo: "#252524",
  capTop: "#605f5c",
  capMid: "#3c3c3a",
  capBottom: "#33332f",
  headerTop: "#2e2e2d",
  headerBottom: "#2a2a29",
  headerSelectedTop: "#3a3a39",
  headerSelectedBottom: "#333332",
  headerAgentTop: "#2f3232",
  headerAgentBottom: "#2b2e2e",
  insertTop: "#3a3a39",
  insertBottom: "#313130",
  chipTop: "#3a3a39",
  chipBottom: "#2c2c2b",
  trafficLight: "#4a4a47",

  // Ink
  inkBright: "#f2f1ee", // transport digits, glass text
  ink100: "#e8e7e4", // primary text
  inkControl: "#d6d5d1", // raised button labels
  inkDim: "#c8c7c3", // SMPTE
  ink300: "#a9a8a4", // secondary
  ink500: "#7f7e7a", // labels, dim
  inkSeparator: "#5a5957", // dots between position fields
  inkKeyWhite: "#4a4a47",

  // Piano keys
  keyWhite: "#d9d7d1",
  keyBlack: "#232322",

  // LEDs and accent
  led: "#f0e8d8",
  ledOff: "#252523",
  ledGlow: "rgba(255,236,205,.75)",
  ledGlowSoft: "rgba(255,236,205,.3)",
  accent: "oklch(0.80 0.12 190)",
  accentHi: "oklch(0.88 0.11 190)",
  accentLo: "oklch(0.66 0.12 190)",
  accentInk: "#0f2a29",
  /** Piano-roll note colour is the bass track hue at two lightnesses. */
  noteTop: "oklch(0.80 0.12 300)",
  noteBottom: "oklch(0.66 0.13 300)",
  eqCurve: "oklch(0.78 0.12 300)",
  eqHandle: "oklch(0.85 0.1 300)",
  eqFill: "oklch(0.72 0.13 300)",

  /** Neutral dot for non-Ondera browser items and neutral log entries. */
  neutralDot: "#5a5a57",

  // Popup menus (title-bar menus and context menus): a raised graphite card.
  menu: "#333332",
  menuHover: "#3f3f3d",
  menuSeparator: "#262625",
} as const;

/** Translucent whites and blacks used by the material recipes. */
export const white = (a: number) => `rgba(255,255,255,${a})`;
export const black = (a: number) => `rgba(0,0,0,${a})`;
export const accentAlpha = (a: number) => `oklch(0.80 0.12 190 / ${a})`;

// ---------------------------------------------------------------------------
// Type
// ---------------------------------------------------------------------------

export const font = {
  ui: "'Manrope', system-ui, -apple-system, sans-serif",
  mono: "'IBM Plex Mono', ui-monospace, Menlo, monospace",
} as const;

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

export const tracking = {
  caps: "0.09em",
  capsWide: "0.1em",
  kind: "0.05em",
} as const;

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

export const radius = {
  xs: 2,
  sm: 3,
  clip: 4,
  button: 4,
  md: 5,
  control: 6,
  lg: 7,
  glass: 8,
  window: 10,
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

// ---------------------------------------------------------------------------
// Material recipes. Light from directly above: specular highlight on the top
// edge, gradient face darkening downward, contact line on the bottom edge,
// then a soft drop shadow at 2–3× the contact distance. Pressed states swap
// the drop shadow for an inner shadow and darken the face.
// ---------------------------------------------------------------------------

export const gradient = {
  raised: `linear-gradient(180deg, ${color.controlTop}, ${color.controlBottom})`,
  pressed: `linear-gradient(180deg, ${color.pressedTop}, ${color.pressedBottom})`,
  lit: `radial-gradient(circle at 50% 30%, ${color.accentHi}, ${color.accentLo})`,
  transport: `linear-gradient(180deg, ${color.transportTop}, ${color.transportBottom})`,
  segment: `linear-gradient(180deg, ${color.segmentTop}, ${color.segmentBottom})`,
  thumb: `linear-gradient(180deg, ${color.thumbTop}, ${color.thumbBottom})`,
  knob: `radial-gradient(circle at 50% 28%, ${color.knobHi}, ${color.knobLo} 72%)`,
  knobBig: `radial-gradient(circle at 50% 25%, ${color.knobBigHi}, ${color.knobBigLo} 70%)`,
  knobInner: `radial-gradient(circle at 50% 35%, ${color.knobInnerHi}, ${color.knobInnerLo})`,
  faderCap: `linear-gradient(180deg, ${color.capTop}, ${color.capMid} 55%, ${color.capBottom})`,
  header: `linear-gradient(180deg, ${color.headerTop}, ${color.headerBottom})`,
  headerSelected: `linear-gradient(180deg, ${color.headerSelectedTop}, ${color.headerSelectedBottom})`,
  headerAgent: `linear-gradient(180deg, ${color.headerAgentTop}, ${color.headerAgentBottom})`,
  insert: `linear-gradient(180deg, ${color.insertTop}, ${color.insertBottom})`,
  chip: `linear-gradient(180deg, ${color.chipTop}, ${color.chipBottom})`,
  sendKey: `radial-gradient(circle at 50% 30%, oklch(0.86 0.11 190), oklch(0.68 0.12 190))`,
  note: `linear-gradient(180deg, ${color.noteTop}, ${color.noteBottom})`,
  eqGrid: `repeating-linear-gradient(90deg, ${white(0.05)} 0 1px, transparent 1px 42px), linear-gradient(180deg, transparent 36px, ${white(0.08)} 36px, ${white(0.08)} 37px, transparent 37px)`,
} as const;

export const shadow = {
  raised: `inset 0 1px 0 ${white(0.09)}, inset 0 -1px 0 ${black(0.35)}, 0 1px 2px ${black(0.55)}, 0 3px 5px ${black(0.25)}`,
  raisedSm: `inset 0 1px 0 ${white(0.09)}, 0 1px 2px ${black(0.55)}`,
  pressed: `inset 0 2px 4px ${black(0.65)}, inset 0 1px 1px ${black(0.5)}, 0 1px 0 ${white(0.05)}`,
  lit: `inset 0 1px 0 ${white(0.45)}, inset 0 -1px 0 ${black(0.3)}, 0 1px 2px ${black(0.6)}, 0 0 12px ${accentAlpha(0.55)}, 0 0 22px ${accentAlpha(0.25)}`,
  knob: `inset 0 1px 0 ${white(0.16)}, inset 0 -1px 1px ${black(0.6)}, 0 2px 3px ${black(0.6)}, 0 4px 7px ${black(0.3)}`,
  knobBig: `inset 0 1px 0 ${white(0.18)}, inset 0 -2px 2px ${black(0.6)}, 0 3px 4px ${black(0.65)}, 0 8px 12px ${black(0.35)}`,
  knobInner: `inset 0 1px 1px ${black(0.7)}, 0 1px 0 ${white(0.08)}`,
  knobIndicator: `inset 0 0 1px ${black(0.9)}, 0 0 0 1px ${black(0.35)}`,
  faderCap: `inset 0 1px 0 ${white(0.25)}, inset 0 -1px 0 ${black(0.5)}, 0 2px 3px ${black(0.7)}, 0 5px 8px ${black(0.35)}`,
  thumb: `inset 0 1px 0 ${white(0.22)}, inset 0 -1px 0 ${black(0.4)}, 0 1px 2px ${black(0.6)}, 0 3px 4px ${black(0.3)}`,
  groove: `inset 0 1px 3px ${black(0.9)}, inset 0 0 0 1px ${black(0.5)}, 0 1px 0 ${white(0.05)}`,
  grooveSoft: `inset 0 1px 2px ${black(0.85)}, 0 1px 0 ${white(0.05)}`,
  grooveShallow: `inset 0 1px 3px ${black(0.8)}, 0 1px 0 ${white(0.05)}`,
  grooveSend: `inset 0 1px 2px ${black(0.6)}, 0 1px 0 ${white(0.04)}`,
  wellDeep: `inset 0 2px 5px ${black(0.85)}, inset 0 0 0 1px ${black(0.6)}, 0 1px 0 ${white(0.06)}`,
  wellInput: `inset 0 2px 4px ${black(0.8)}, inset 0 0 0 1px ${black(0.6)}, 0 1px 0 ${white(0.05)}`,
  wellValue: `inset 0 1px 3px ${black(0.85)}`,
  segment: `inset 0 1px 0 ${white(0.1)}, 0 1px 2px ${black(0.5)}`,
  clip: `inset 0 1px 0 ${white(0.22)}, inset 0 -1px 0 ${black(0.4)}, 0 2px 3px ${black(0.55)}, 0 5px 10px ${black(0.3)}`,
  clipAgent: `0 0 0 1px ${color.accent}, 0 0 14px ${accentAlpha(0.45)}`,
  clipSelected: `0 0 0 1.5px ${white(0.85)}`,
  ledLit: `0 0 4px ${color.ledGlow}, 0 0 9px ${color.ledGlowSoft}`,
  ledAccent: `0 0 5px ${accentAlpha(0.9)}, 0 0 10px ${accentAlpha(0.4)}`,
  ledOff: `inset 0 1px 0 ${black(0.6)}`,
  ledEmpty: `inset 0 0 0 1px ${white(0.12)}`,
  glass: `inset 0 1px 0 ${white(0.15)}, 0 0 0 1px ${black(0.4)}, 0 3px 8px ${black(0.4)}`,
  glassAgent: `inset 0 1px 0 ${white(0.18)}, 0 0 0 1px ${accentAlpha(0.45)}, 0 4px 12px ${black(0.5)}, 0 0 18px ${accentAlpha(0.18)}`,
  glassCard: `inset 0 1px 0 ${white(0.14)}, 0 0 0 1px ${accentAlpha(0.35)}, 0 6px 18px ${black(0.45)}, 0 0 24px ${accentAlpha(0.12)}`,
  accentDot: `0 0 6px ${accentAlpha(0.9)}, 0 0 12px ${accentAlpha(0.4)}`,
  accentDotLg: `0 0 6px ${accentAlpha(0.9)}, 0 0 14px ${accentAlpha(0.45)}`,
  accentBar: `0 0 6px ${accentAlpha(0.7)}`,
  playhead: `0 0 5px ${accentAlpha(0.7)}, 0 0 14px ${accentAlpha(0.25)}`,
  playheadRuler: `0 0 6px ${accentAlpha(0.8)}`,
  headerCell: `inset -1px 0 0 ${black(0.6)}, inset 0 -1px 0 ${black(0.55)}, inset 0 1px 0 ${white(0.04)}`,
  headerCellAgent: `inset 0 0 0 1px ${accentAlpha(0.55)}, inset 0 0 18px ${accentAlpha(0.12)}, inset -1px 0 0 ${black(0.6)}, inset 0 -1px 0 ${black(0.55)}, inset 0 1px 0 ${white(0.04)}`,
  headerColumn: `inset -1px 0 0 ${black(0.6)}`,
  colorStrip: `inset -1px 0 0 ${black(0.4)}, inset 1px 0 0 ${white(0.15)}`,
  swatch: `inset 0 1px 0 ${white(0.25)}, 0 1px 1px ${black(0.5)}`,
  swatchSm: `inset 0 1px 0 ${white(0.2)}, 0 1px 1px ${black(0.5)}`,
  trafficLight: `inset 0 1px 0 ${white(0.12)}, 0 1px 1px ${black(0.5)}`,
  titleBar: `inset 0 -1px 0 ${black(0.5)}, inset 0 1px 0 ${white(0.05)}`,
  transport: `inset 0 1px 0 ${white(0.06)}, 0 2px 6px ${black(0.45)}`,
  toolbar: `inset 0 -1px 0 ${black(0.6)}, 0 1px 0 ${white(0.03)}`,
  toolbarEditor: `inset 0 -1px 0 ${black(0.6)}`,
  rulerCorner: `inset -1px 0 0 ${black(0.6)}, inset 0 -1px 0 ${black(0.6)}`,
  panelLeft: `inset -1px 0 0 ${black(0.6)}, inset -2px 0 4px ${black(0.25)}`,
  panelRight: `inset 1px 0 0 ${black(0.6)}, inset 2px 0 4px ${black(0.25)}`,
  panelAgent: `inset 1px 0 0 ${black(0.65)}, inset 3px 0 6px ${black(0.3)}`,
  panelAgentRail: `inset 1px 0 0 ${black(0.65)}`,
  editorPane: `0 -3px 10px ${black(0.5)}, inset 0 1px 0 ${white(0.06)}`,
  keyColumn: `inset -1px 0 0 ${black(0.7)}`,
  keyRow: `inset 0 -1px 0 ${black(0.35)}`,
  divider: `inset 0 -1px 0 ${black(0.5)}`,
  dividerTop: `inset 0 1px 0 ${white(0.04)}`,
  logEntry: `inset 0 1px 0 ${white(0.04)}, 0 1px 2px ${black(0.35)}`,
  logChip: `inset 0 1px 0 ${white(0.12)}, 0 1px 2px ${black(0.5)}`,
  insert: `inset 0 1px 0 ${white(0.08)}, 0 1px 2px ${black(0.5)}`,
  insertEmpty: `inset 0 1px 2px ${black(0.6)}, 0 1px 0 ${white(0.04)}`,
  ledInsertOff: `inset 0 1px 1px ${black(0.8)}`,
  window: `0 30px 80px ${black(0.6)}, 0 0 0 1px ${white(0.06)}`,
  sendKey: `inset 0 1px 0 ${white(0.4)}, inset 0 -1px 0 ${black(0.25)}, 0 1px 2px ${black(0.6)}, 0 0 12px ${accentAlpha(0.4)}`,
  menu: `inset 0 1px 0 ${white(0.08)}, 0 0 0 1px ${black(0.6)}, 0 8px 24px ${black(0.55)}, 0 2px 6px ${black(0.4)}`,
  inlineInput: `inset 0 1px 2px ${black(0.7)}, 0 0 0 1px ${accentAlpha(0.6)}`,
  note: `inset 0 1px 0 ${white(0.35)}, inset 0 -1px 0 ${black(0.3)}, 0 1px 2px ${black(0.6)}, 0 2px 4px ${black(0.3)}`,
  /** Milled slot on a fader cap: dark line with a 1 px light edge below. */
  faderLineHi: `0 1px 0 ${white(0.14)}`,
  /** Milled slot on a slider thumb: dark line with a 1 px light edge to the right. */
  milledHi: `1px 0 0 ${white(0.12)}`,
} as const;

/**
 * Shadow recipes for canvas code, which cannot parse CSS box-shadow strings.
 * Each entry is a list of layers painted back to front.
 */
export interface CanvasShadowLayer {
  blur: number;
  offsetY: number;
  color: string;
}

export const canvasShadow = {
  clipDrop: [
    { blur: 10, offsetY: 5, color: black(0.3) },
    { blur: 3, offsetY: 2, color: black(0.55) },
  ],
  playhead: [
    { blur: 14, offsetY: 0, color: accentAlpha(0.25) },
    { blur: 5, offsetY: 0, color: accentAlpha(0.7) },
  ],
  playheadRuler: [{ blur: 6, offsetY: 0, color: accentAlpha(0.8) }],
  playheadFlag: [{ blur: 4, offsetY: 0, color: accentAlpha(0.7) }],
  bubble: [{ blur: 8, offsetY: 3, color: black(0.4) }],
  agentRing: [{ blur: 14, offsetY: 0, color: accentAlpha(0.45) }],
  agentNote: [
    { blur: 10, offsetY: 0, color: accentAlpha(0.4) },
    { blur: 5, offsetY: 0, color: accentAlpha(0.9) },
  ],
  noteSelected: [{ blur: 6, offsetY: 0, color: white(0.5) }],
  note: [
    { blur: 4, offsetY: 2, color: black(0.3) },
    { blur: 2, offsetY: 1, color: black(0.6) },
  ],
} as const satisfies Record<string, readonly CanvasShadowLayer[]>;

/** Mix ratios (percent of track colour against the panel colour) for clip faces. */
export const clipMix = {
  faceTop: 66,
  faceBottom: 50,
} as const;

/** Translucent fills used by chrome that is not a full material. */
export const fill = {
  glass: "rgba(60,60,58,.55)",
  glassAgent: "rgba(58,64,64,.55)",
  glassCard: "rgba(66,70,70,.4)",
  logLive: "rgba(66,70,70,.35)",
  logChipLive: black(0.25),
  browserHighlight: white(0.07),
  cycleRuler: white(0.09),
  cycleLane: white(0.025),
  clipTitle: black(0.22),
  rowShade: white(0.035),
  blackKeyRow: black(0.14),
  eqGridLine: white(0.05),
  eqZeroLine: white(0.08),
  velocity: white(0.18),
  /** Ghost of a clip or note while it is being dragged. */
  dragGhost: white(0.12),
  /** Marquee / pencil preview while drawing a new clip or note. */
  pencilPreview: accentAlpha(0.25),
  cycleHandle: white(0.35),
  dropTarget: accentAlpha(0.12),
  stepCell: white(0.06),
} as const;

/** Line colours for grids and rules. */
export const line = {
  barLine: white(0.075),
  beatLine: white(0.025),
  rulerBar: white(0.16),
  rulerTick: white(0.07),
  cycleEdge: white(0.2),
  editorBar: white(0.09),
  editorBeat: white(0.03),
  waveform: white(0.72),
  waveformMid: white(0.35),
  midiNote: white(0.78),
  clipName: white(0.88),
  clipTitleBottom: black(0.2),
  cellDivider: white(0.06),
  dividerDark: black(0.5),
  laneTop: white(0.02),
  laneBottom: black(0.45),
  rulerBottom: black(0.6),
  faderLine: black(0.7),
  faderLineHi: white(0.14),
  milled: black(0.6),
  milledHi: white(0.12),
  dragGhostEdge: white(0.5),
  noteSelected: white(0.9),
  splitGuide: accentAlpha(0.9),
  staff: white(0.35),
  noteHead: white(0.9),
} as const;

export const blur = {
  glass: "8px",
  glassAgent: "10px",
  glassCard: "12px",
} as const;

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

/** Every token as a CSS custom property. Applied once at startup by applyTokens(). */
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
  return vars;
}

export const tokens = {
  color,
  font,
  fontSize,
  fontWeight,
  tracking,
  lineHeight,
  space,
  radius,
  size,
  timeline,
  gradient,
  shadow,
  fill,
  line,
  blur,
  canvasShadow,
  clipMix,
  white,
  black,
  accentAlpha,
} as const;

export type Tokens = typeof tokens;
