/**
 * ryolune: the one theme, in a dark and a light mode.
 *
 * Studio hardware drawn with restraint. Depth comes from a calm lightness ladder, crisp
 * one-pixel edges and a single soft light from above: every control has a fine top
 * highlight, a dark contact edge and a short drop, never a gloss or a texture. One warm
 * accent, lunar gold, marks the playhead, what is lit and what the agent touched; the
 * meters keep their own mint and amber so levels never read as "selected".
 *
 * Dark is graphite at night (neutral hue 268, chroma under 0.008), displays sunk into it.
 * Light is porcelain under daylight: the same ladder mirrored, white controls standing
 * proud of a pale desk, shade at a third of the dark strength for the same depth.
 *
 * Ladder, dark:  desk .135 · well .15 · lanes .185 · panel .205 · card .225 · control .27
 * Ladder, light: desk .905 · groove .89 · panel .968 · lanes .988 · control 1
 */
import {
  alpha,
  black,
  families,
  ok,
  white,
  type CanvasShadowKey,
  type CanvasShadowLayer,
  type ColorKey,
  type FillKey,
  type GradientKey,
  type LineKey,
  type Mode,
  type ShadowKey,
  type ThemeSpec,
} from "./schema";

const FONT = "'Manrope', system-ui, -apple-system, sans-serif";
const RADIUS = {
  xs: 2,
  sm: 4,
  clip: 5,
  button: 6,
  md: 6,
  control: 7,
  lg: 9,
  glass: 12,
  window: 14,
};

/** The EQ display's grid: a line every 42 px and the 0 dB line 36 px down. */
const eqGrid = (grid: string, zero: string) =>
  `repeating-linear-gradient(90deg, ${grid} 0 1px, transparent 1px 42px), linear-gradient(180deg, transparent 36px, ${zero} 36px, ${zero} 37px, transparent 37px)`;

const HUE = 268;
/** The neutral: graphite by night, porcelain by day. */
const n = (L: number, C = 0.006) => ok(L, C, HUE);
/** The porcelain neutral of the light mode: the same hue, less chroma. */
const l = (L: number, C = 0.004) => n(L, C);
/** Lunar gold, the accent. By day it leans toward amber so it never reads as mustard. */
const gold = (L: number, C: number, a = 1) => ok(L, C, 76, a);
const amber = (L: number, C: number, a = 1) => ok(L, C, 64, a);

function colors(mode: Mode): Record<ColorKey, string> {
  if (mode === "dark") {
    const accent = gold(0.83, 0.115);
    return {
      desk: n(0.135),
      wellDeep: n(0.148, 0.005),
      groove: n(0.14),
      grooveAlt: n(0.165),
      timelineEmpty: n(0.168),
      timeline: n(0.186),
      timelineAgent: gold(0.2, 0.012),
      timelineSelected: n(0.212),
      editor: n(0.18),
      ruler: n(0.198),
      materialCard: n(0.226),
      agentPanel: n(0.196),
      panel: n(0.205),
      logEntry: n(0.228),
      raised: n(0.248),
      controlFace: n(0.31),
      controlTop: n(0.282),
      controlBottom: n(0.258),
      pressedTop: n(0.158),
      pressedBottom: n(0.176),
      transportTop: n(0.222),
      transportBottom: n(0.204),
      segmentTop: n(0.318),
      segmentBottom: n(0.29),
      thumbTop: n(0.97, 0.003),
      thumbBottom: n(0.84, 0.004),
      knobHi: n(0.35),
      knobLo: n(0.235),
      knobBigHi: n(0.37),
      knobBigLo: n(0.235),
      knobInnerHi: n(0.27),
      knobInnerLo: n(0.215),
      capTop: n(0.95, 0.003),
      capMid: n(0.86, 0.004),
      capBottom: n(0.76, 0.005),
      headerTop: n(0.214),
      headerBottom: n(0.204),
      headerSelectedTop: n(0.258),
      headerSelectedBottom: n(0.246),
      headerAgentTop: gold(0.235, 0.016),
      headerAgentBottom: gold(0.222, 0.014),
      insertTop: n(0.262),
      insertBottom: n(0.245),
      chipTop: n(0.27),
      chipBottom: n(0.252),
      trafficLight: n(0.33),
      inkBright: n(0.975, 0.002),
      ink100: n(0.93, 0.003),
      inkControl: n(0.885, 0.004),
      inkDim: n(0.81, 0.005),
      ink300: n(0.735, 0.007),
      ink500: n(0.625, 0.009),
      inkSeparator: n(0.44, 0.01),
      inkKeyWhite: n(0.45),
      wellInk: n(0.975, 0.002),
      wellInkDim: n(0.74, 0.007),
      wellInkFaint: n(0.6, 0.009),
      keyWhite: n(0.92, 0.003),
      keyBlack: n(0.2),
      led: ok(0.84, 0.15, 158),
      ledOff: n(0.235),
      ledGlow: ok(0.84, 0.15, 158, 0.55),
      ledGlowSoft: ok(0.84, 0.15, 158, 0.2),
      ledHot: ok(0.8, 0.15, 62),
      accent,
      accentHi: gold(0.89, 0.1),
      accentLo: gold(0.74, 0.12),
      accentInk: gold(0.2, 0.035),
      noteTop: ok(0.8, 0.105, 285),
      noteBottom: ok(0.72, 0.12, 285),
      eqCurve: accent,
      eqHandle: n(0.97, 0.003),
      eqFill: accent,
      ...families(0.78, 0.115),
      neutralDot: n(0.46),
      menu: n(0.238),
      menuHover: n(0.305),
      menuSeparator: n(0.29),
      indicator: n(0.97, 0.003),
      danger: ok(0.7, 0.18, 24),
      mute: ok(0.76, 0.1, 245),
      solo: ok(0.86, 0.14, 95),
      toneInk: n(0.17, 0.01),
      scrim: black(0.48),
    };
  }
  const accent = amber(0.62, 0.14);
  return {
    desk: l(0.905),
    wellDeep: l(0.948, 0.004),
    groove: l(0.89),
    grooveAlt: l(0.962),
    timelineEmpty: l(0.952),
    timeline: l(0.988, 0.002),
    timelineAgent: gold(0.972, 0.018),
    timelineSelected: l(0.962),
    editor: l(0.988, 0.002),
    ruler: l(0.972),
    materialCard: l(1, 0),
    agentPanel: l(0.982, 0.003),
    panel: l(0.968),
    logEntry: l(1, 0),
    raised: l(1, 0),
    controlFace: l(0.87),
    controlTop: l(1, 0),
    controlBottom: l(0.982, 0.003),
    pressedTop: l(0.9),
    pressedBottom: l(0.925),
    transportTop: l(0.985, 0.002),
    transportBottom: l(0.968),
    segmentTop: l(1, 0),
    segmentBottom: l(0.985, 0.002),
    thumbTop: l(1, 0),
    thumbBottom: l(0.955),
    knobHi: l(1, 0),
    knobLo: l(0.925),
    knobBigHi: l(1, 0),
    knobBigLo: l(0.92),
    knobInnerHi: l(0.985, 0.002),
    knobInnerLo: l(0.95),
    capTop: l(1, 0),
    capMid: l(0.975, 0.002),
    capBottom: l(0.93),
    headerTop: l(0.972),
    headerBottom: l(0.965),
    headerSelectedTop: l(0.94),
    headerSelectedBottom: l(0.932),
    headerAgentTop: gold(0.955, 0.03),
    headerAgentBottom: gold(0.945, 0.03),
    insertTop: l(1, 0),
    insertBottom: l(0.982, 0.003),
    chipTop: l(0.955),
    chipBottom: l(0.94),
    trafficLight: l(0.84),
    inkBright: l(0.13, 0.01),
    ink100: l(0.2, 0.01),
    inkControl: l(0.28, 0.01),
    inkDim: l(0.36, 0.01),
    ink300: l(0.45, 0.012),
    ink500: l(0.55, 0.012),
    inkSeparator: l(0.8, 0.006),
    inkKeyWhite: l(0.55),
    wellInk: l(0.13, 0.01),
    wellInkDim: l(0.42, 0.012),
    wellInkFaint: l(0.54, 0.012),
    keyWhite: l(1, 0),
    keyBlack: l(0.24, 0.008),
    led: ok(0.66, 0.15, 156),
    ledOff: l(0.87),
    ledGlow: "transparent",
    ledGlowSoft: "transparent",
    ledHot: ok(0.7, 0.16, 58),
    accent,
    accentHi: amber(0.69, 0.135),
    accentLo: amber(0.57, 0.14),
    accentInk: "#ffffff",
    noteTop: ok(0.6, 0.15, 280),
    noteBottom: ok(0.54, 0.16, 280),
    eqCurve: accent,
    eqHandle: accent,
    eqFill: accent,
    ...families(0.55, 0.13),
    neutralDot: l(0.72),
    menu: l(1, 0),
    menuHover: l(0.952),
    menuSeparator: l(0.92),
    indicator: l(0.22, 0.01),
    danger: ok(0.57, 0.2, 26),
    mute: ok(0.6, 0.12, 248),
    solo: ok(0.8, 0.15, 92),
    toneInk: l(0.16, 0.01),
    scrim: "rgba(16,18,26,.24)",
  };
}

export function ryolune(mode: Mode): ThemeSpec {
  const dark = mode === "dark";
  const c = colors(mode);
  // `ink(a)` tints and rules in the mode's foreground; `lo(a)` is shade, `hi(a)` light.
  const ink = dark ? white : (a: number) => `rgba(20,22,32,${a})`;
  const lo = dark
    ? black
    : (a: number) => `rgba(20,22,40,${Number((a * 0.32).toFixed(3))})`;
  const hi = dark ? white : (a: number) => white(Math.min(1, 0.6 + a * 2));
  const ac = (a: number) => alpha(c.accent, a);
  const v = (a: string, b: string) => `linear-gradient(180deg, ${a}, ${b})`;

  // A dark edge reads crisper than a light one on graphite; on porcelain a soft grey line.
  const edge = dark ? black(0.5) : "rgba(20,22,40,.13)";
  const edgeSoft = dark ? black(0.36) : "rgba(20,22,40,.09)";
  const border = ink(dark ? 0.075 : 0.095);
  const borderStrong = ink(dark ? 0.13 : 0.16);
  const hairline = ink(dark ? 0.045 : 0.06);
  // Panels meet on a dark seam with a lit lip beside it, like two plates butting.
  const seam = dark ? black(0.55) : "rgba(20,22,40,.1)";
  const lip = dark ? white(0.035) : white(0.7);

  const fill: Record<FillKey, string> = {
    glass: dark ? "rgba(34,35,42,.8)" : "rgba(255,255,255,.82)",
    glassAgent: dark ? "rgba(38,37,36,.84)" : "rgba(255,253,248,.88)",
    glassCard: dark ? "rgba(40,40,46,.88)" : "rgba(255,255,255,.92)",
    logLive: ac(dark ? 0.08 : 0.07),
    logChipLive: ac(dark ? 0.15 : 0.12),
    browserHighlight: ink(dark ? 0.065 : 0.05),
    cycleRuler: ac(dark ? 0.24 : 0.2),
    cycleLane: ac(dark ? 0.04 : 0.045),
    clipTitle: dark ? black(0.2) : white(0.38),
    rowShade: ink(dark ? 0.022 : 0.02),
    blackKeyRow: dark ? black(0.18) : "rgba(20,22,32,.035)",
    eqGridLine: ink(dark ? 0.05 : 0.06),
    eqZeroLine: ink(dark ? 0.12 : 0.14),
    velocity: ink(dark ? 0.24 : 0.3),
    dragGhost: ink(dark ? 0.09 : 0.08),
    pencilPreview: ac(0.24),
    cycleHandle: ac(0.95),
    dropTarget: ac(dark ? 0.09 : 0.1),
    stepCell: ink(dark ? 0.05 : 0.05),
    hover: ink(dark ? 0.055 : 0.042),
    fadeShade: dark ? black(0.38) : "rgba(20,22,32,.14)",
    fadeHandle: dark ? white(0.94) : "rgba(20,22,32,.82)",
    markerFlag: dark ? "rgba(22,23,28,.9)" : "rgba(255,255,255,.94)",
  };

  const markerHue = 222;
  const line: Record<LineKey, string> = {
    barLine: ink(dark ? 0.07 : 0.095),
    beatLine: ink(dark ? 0.026 : 0.038),
    rulerBar: ink(dark ? 0.24 : 0.3),
    rulerTick: ink(dark ? 0.1 : 0.13),
    cycleEdge: ac(0.85),
    editorBar: ink(dark ? 0.08 : 0.105),
    editorBeat: ink(dark ? 0.028 : 0.042),
    waveform: dark ? white(0.82) : "rgba(20,22,32,.7)",
    waveformMid: dark ? white(0.28) : "rgba(20,22,32,.24)",
    midiNote: dark ? white(0.84) : "rgba(20,22,32,.72)",
    clipName: dark ? white(0.95) : "rgba(20,22,32,.9)",
    clipTitleBottom: dark ? black(0.12) : "rgba(20,22,32,.05)",
    clipHighlight: dark ? white(0.16) : white(0.75),
    clipContact: dark ? black(0.32) : "rgba(20,22,32,.1)",
    clipSelected: c.inkBright,
    cellDivider: hairline,
    dividerDark: border,
    laneTop: dark ? white(0.018) : white(0.6),
    laneBottom: dark ? black(0.32) : "rgba(20,22,32,.07)",
    rulerBottom: seam,
    faderLine: dark ? "rgba(20,22,32,.6)" : "rgba(20,22,32,.4)",
    faderLineHi: dark ? white(0.4) : white(0.9),
    milled: dark ? black(0.35) : "rgba(20,22,32,.25)",
    milledHi: dark ? white(0.5) : white(0.9),
    dragGhostEdge: ink(0.42),
    noteSelected: c.inkBright,
    noteHighlight: dark ? white(0.28) : white(0.4),
    splitGuide: ac(0.95),
    staff: ink(dark ? 0.3 : 0.34),
    noteHead: c.ink100,
    border,
    borderStrong,
    hairline,
    fadeCurve: dark ? white(0.86) : "rgba(20,22,32,.72)",
    marker: dark ? ok(0.8, 0.1, markerHue) : ok(0.55, 0.13, markerHue),
    markerLane: dark
      ? ok(0.8, 0.1, markerHue, 0.28)
      : ok(0.55, 0.13, markerHue, 0.3),
  };

  const shadow: Record<ShadowKey, string> = {
    raised: `inset 0 1px 0 ${hi(0.075)}, 0 0 0 1px ${edge}, 0 1px 2px ${lo(0.3)}`,
    raisedSm: `inset 0 1px 0 ${hi(0.065)}, 0 0 0 1px ${edge}, 0 1px 1px ${lo(0.22)}`,
    pressed: `inset 0 1px 2px ${lo(0.5)}, inset 0 0 0 1px ${edge}, 0 1px 0 ${hi(0.04)}`,
    lit: `inset 0 1px 0 ${white(dark ? 0.42 : 0.35)}, inset 0 -1px 0 ${black(0.12)}, 0 0 0 1px ${alpha(c.accentLo, dark ? 0.9 : 1)}, 0 1px 2px ${lo(0.35)}, 0 0 16px ${ac(dark ? 0.28 : 0.18)}`,
    knob: `inset 0 1px 0 ${hi(0.14)}, inset 0 -1px 1px ${lo(0.35)}, 0 0 0 1px ${edge}, 0 2px 4px ${lo(0.45)}`,
    knobBig: `inset 0 1px 0 ${hi(0.16)}, inset 0 -2px 2px ${lo(0.35)}, 0 0 0 1px ${edge}, 0 3px 8px ${lo(0.5)}`,
    knobInner: `inset 0 1px 1px ${lo(0.45)}, 0 1px 0 ${hi(0.06)}`,
    knobIndicator: dark ? `0 0 4px ${white(0.25)}` : "none",
    faderCap: `inset 0 1px 0 ${white(0.9)}, inset 0 -1px 0 ${black(0.12)}, 0 0 0 1px ${edge}, 0 2px 4px ${lo(0.5)}, 0 6px 12px ${lo(0.25)}`,
    thumb: `inset 0 -1px 0 ${black(0.1)}, 0 0 0 1px ${edge}, 0 1px 3px ${lo(0.45)}`,
    groove: `inset 0 1px 2px ${lo(0.55)}, inset 0 0 0 1px ${edgeSoft}, 0 1px 0 ${hi(0.035)}`,
    grooveSoft: `inset 0 1px 1px ${lo(0.5)}, inset 0 0 0 1px ${edgeSoft}`,
    grooveShallow: `inset 0 1px 2px ${lo(0.3)}, inset 0 0 0 1px ${edgeSoft}, 0 1px 0 ${hi(0.035)}`,
    grooveSend: `inset 0 1px 1px ${lo(0.4)}, inset 0 0 0 1px ${edgeSoft}`,
    wellDeep: `inset 0 1px 3px ${lo(dark ? 0.55 : 0.4)}, inset 0 0 0 1px ${edge}, 0 1px 0 ${hi(0.045)}`,
    wellInput: `inset 0 1px 2px ${lo(0.35)}, inset 0 0 0 1px ${edgeSoft}, 0 1px 0 ${hi(0.03)}`,
    wellValue: `inset 0 1px 2px ${lo(0.4)}`,
    segment: `inset 0 1px 0 ${hi(0.08)}, 0 0 0 1px ${edge}, 0 1px 2px ${lo(0.3)}`,
    clip: `inset 0 1px 0 ${dark ? white(0.16) : white(0.75)}, 0 0 0 1px ${lo(0.25)}, 0 1px 2px ${lo(0.3)}`,
    clipAgent: `0 0 0 1px ${c.accent}, 0 0 12px ${ac(0.35)}`,
    clipSelected: `0 0 0 1.5px ${c.inkBright}`,
    ledLit: dark ? `0 0 4px ${c.ledGlow}` : "none",
    ledAccent: dark ? `0 0 5px ${alpha(c.ledHot, 0.6)}` : "none",
    ledOff: `inset 0 1px 0 ${lo(0.3)}`,
    ledEmpty: `inset 0 0 0 1px ${border}`,
    glass: `inset 0 1px 0 ${hi(0.06)}, 0 0 0 1px ${edge}, 0 8px 24px ${lo(0.4)}`,
    glassAgent: `inset 0 1px 0 ${hi(0.07)}, 0 0 0 1px ${ac(0.45)}, 0 8px 24px ${lo(0.4)}, 0 0 18px ${ac(0.1)}`,
    glassCard: `inset 0 1px 0 ${hi(0.06)}, 0 0 0 1px ${ac(0.35)}, 0 10px 30px ${lo(0.42)}`,
    accentDot: `0 0 0 3px ${ac(0.16)}, 0 0 8px ${ac(dark ? 0.55 : 0.3)}`,
    accentDotLg: `0 0 0 4px ${ac(0.16)}, 0 0 10px ${ac(dark ? 0.55 : 0.3)}`,
    accentBar: `0 0 8px ${ac(dark ? 0.5 : 0.25)}`,
    playhead: `0 0 6px ${ac(dark ? 0.55 : 0.3)}`,
    playheadRuler: `0 0 6px ${ac(dark ? 0.6 : 0.3)}`,
    headerCell: `inset -1px 0 0 ${seam}, inset 0 -1px 0 ${line.laneBottom}, inset 0 1px 0 ${line.laneTop}`,
    headerCellAgent: `inset 2px 0 0 ${c.accent}, inset -1px 0 0 ${seam}, inset 0 -1px 0 ${line.laneBottom}, inset 0 1px 0 ${line.laneTop}`,
    headerColumn: `inset -1px 0 0 ${seam}`,
    colorStrip: `inset -1px 0 0 ${black(0.18)}, inset 1px 0 0 ${white(0.18)}`,
    swatch: `inset 0 0 0 1px ${black(0.2)}, inset 0 1px 0 ${white(0.25)}`,
    swatchSm: `inset 0 0 0 1px ${black(0.18)}`,
    trafficLight: `inset 0 0 0 1px ${lo(0.25)}`,
    titleBar: `inset 0 -1px 0 ${seam}`,
    transport: `inset 0 1px 0 ${lip}, inset 0 -1px 0 ${seam}, 0 1px 3px ${lo(dark ? 0.35 : 0.2)}`,
    toolbar: `inset 0 -1px 0 ${seam}, inset 0 1px 0 ${lip}`,
    toolbarEditor: `inset 0 -1px 0 ${seam}`,
    rulerCorner: `inset -1px 0 0 ${seam}, inset 0 -1px 0 ${seam}`,
    panelLeft: `inset -1px 0 0 ${seam}`,
    panelRight: `inset 1px 0 0 ${seam}`,
    panelAgent: `inset 1px 0 0 ${seam}`,
    panelAgentRail: `inset 1px 0 0 ${seam}`,
    editorPane: `inset 0 1px 0 ${seam}, inset 0 2px 0 ${lip}, 0 -4px 12px ${lo(dark ? 0.3 : 0.15)}`,
    keyColumn: `inset -1px 0 0 ${seam}`,
    keyRow: `inset 0 -1px 0 ${dark ? black(0.3) : "rgba(20,22,32,.08)"}`,
    divider: `inset 0 -1px 0 ${seam}`,
    dividerTop: `inset 0 1px 0 ${lip}`,
    logEntry: `inset 0 1px 0 ${hi(0.035)}, 0 0 0 1px ${edgeSoft}`,
    logChip: `inset 0 1px 0 ${hi(0.06)}, 0 0 0 1px ${edgeSoft}`,
    insert: `inset 0 1px 0 ${hi(0.06)}, 0 0 0 1px ${edge}, 0 1px 1px ${lo(0.2)}`,
    insertEmpty: `inset 0 0 0 1px ${hairline}`,
    ledInsertOff: `inset 0 1px 1px ${lo(0.5)}, inset 0 0 0 1px ${edgeSoft}`,
    window: `0 30px 80px ${lo(0.6)}, 0 0 0 1px ${edge}`,
    sendKey: `inset 0 1px 0 ${white(0.4)}, 0 0 0 1px ${c.accentLo}, 0 0 10px ${ac(0.3)}`,
    menu: `inset 0 1px 0 ${hi(0.07)}, 0 0 0 1px ${edge}, 0 12px 32px ${lo(0.5)}, 0 2px 6px ${lo(0.25)}`,
    dialog: `inset 0 1px 0 ${hi(0.07)}, 0 0 0 1px ${edge}, 0 28px 80px ${lo(0.6)}, 0 6px 18px ${lo(0.3)}`,
    inlineInput: `inset 0 1px 2px ${lo(0.35)}, 0 0 0 1px ${c.accent}, 0 0 0 3px ${ac(0.18)}`,
    note: `inset 0 1px 0 ${white(dark ? 0.3 : 0.35)}, inset 0 0 0 1px ${black(0.18)}`,
    faderLineHi: `0 1px 0 ${hi(0.12)}`,
    milledHi: `1px 0 0 ${hi(0.12)}`,
    plate: `inset 0 1px 0 ${hi(0.05)}, 0 0 0 1px ${edge}, 0 2px 6px ${lo(0.3)}`,
    focus: `0 0 0 1px ${c.accent}, 0 0 0 3px ${ac(0.25)}`,
  };

  const gradient: Record<GradientKey, string> = {
    raised: v(c.controlTop, c.controlBottom),
    raisedHover: dark ? v(n(0.31), n(0.282)) : v(l(1), l(0.968)),
    pressed: v(c.pressedTop, c.pressedBottom),
    lit: v(c.accentHi, c.accentLo),
    transport: v(c.transportTop, c.transportBottom),
    titleBar: v(dark ? n(0.19) : l(0.955), dark ? n(0.175) : l(0.94)),
    segment: v(c.segmentTop, c.segmentBottom),
    thumb: v(c.thumbTop, c.thumbBottom),
    knob: `radial-gradient(circle at 50% 22%, ${c.knobHi}, ${c.knobLo} 78%)`,
    knobBig: `radial-gradient(circle at 50% 20%, ${c.knobBigHi}, ${c.knobBigLo} 76%)`,
    knobInner: `radial-gradient(circle at 50% 30%, ${c.knobInnerHi}, ${c.knobInnerLo})`,
    faderCap: `linear-gradient(180deg, ${c.capTop}, ${c.capMid} 50%, ${c.capBottom})`,
    header: v(c.headerTop, c.headerBottom),
    headerSelected: v(c.headerSelectedTop, c.headerSelectedBottom),
    headerAgent: v(c.headerAgentTop, c.headerAgentBottom),
    panelHeader: v(dark ? n(0.215) : l(0.975), c.panel),
    insert: v(c.insertTop, c.insertBottom),
    chip: v(c.chipTop, c.chipBottom),
    sendKey: v(c.accentHi, c.accentLo),
    note: v(c.noteTop, c.noteBottom),
    eqGrid: eqGrid(fill.eqGridLine, fill.eqZeroLine),
    dialog: v(dark ? n(0.228) : l(1, 0), dark ? n(0.208) : l(0.985, 0.002)),
    dialogHeader: v(
      dark ? n(0.235) : l(1, 0),
      dark ? n(0.222) : l(0.988, 0.002),
    ),
    plate: v(dark ? n(0.232) : l(1, 0), dark ? n(0.214) : l(0.982, 0.003)),
  };

  const none: readonly CanvasShadowLayer[] = [];
  const canvasShadow: Record<CanvasShadowKey, readonly CanvasShadowLayer[]> =
    dark
      ? {
          clipDrop: [
            { blur: 6, offsetY: 2, color: black(0.32) },
            { blur: 1.5, offsetY: 1, color: black(0.45) },
          ],
          playhead: [{ blur: 6, offsetY: 0, color: ac(0.55) }],
          playheadRuler: [{ blur: 6, offsetY: 0, color: ac(0.6) }],
          playheadFlag: [{ blur: 4, offsetY: 0, color: ac(0.5) }],
          bubble: [{ blur: 10, offsetY: 4, color: black(0.4) }],
          agentRing: [{ blur: 10, offsetY: 0, color: ac(0.4) }],
          agentNote: [{ blur: 6, offsetY: 0, color: ac(0.6) }],
          noteSelected: [{ blur: 5, offsetY: 0, color: white(0.35) }],
          note: [{ blur: 2, offsetY: 1, color: black(0.4) }],
        }
      : {
          clipDrop: [{ blur: 3, offsetY: 1, color: "rgba(20,22,40,.12)" }],
          playhead: none,
          playheadRuler: none,
          playheadFlag: none,
          bubble: [{ blur: 10, offsetY: 3, color: "rgba(20,22,40,.14)" }],
          agentRing: [{ blur: 8, offsetY: 0, color: ac(0.3) }],
          agentNote: [{ blur: 5, offsetY: 0, color: ac(0.45) }],
          noteSelected: none,
          note: [{ blur: 2, offsetY: 1, color: "rgba(20,22,40,.14)" }],
        };

  return {
    scheme: mode,
    color: c,
    gradient,
    shadow,
    fill,
    line,
    blur: { glass: "20px", glassAgent: "20px", glassCard: "24px" },
    radius: RADIUS,
    // Quick to answer, soft to land: nothing bounces, nothing lingers.
    motion: {
      fast: "80ms",
      base: "150ms",
      slow: "240ms",
      ease: "cubic-bezier(0.2, 0, 0, 1)",
      enter: "cubic-bezier(0.16, 1, 0.3, 1)",
      exit: "cubic-bezier(0.4, 0, 1, 1)",
      settle: "cubic-bezier(0.22, 1, 0.36, 1)",
    },
    canvasShadow,
    clipMix: dark
      ? { faceTop: 64, faceBottom: 52 }
      : { faceTop: 60, faceBottom: 70 },
    fontUi: FONT,
    capsTracking: "0.08em",
    vars: {
      "--display-glow": dark ? "0.14" : "0",
      "--ring-glow": dark ? ac(0.45) : "transparent",
      // The window's own chrome: the frame behind every panel, a 1 px lip under the
      // title bar and the selection colour of text.
      "--ryo-frame": c.desk,
      "--ryo-seam": seam,
      "--ryo-lip": lip,
      "--ryo-selection": ac(dark ? 0.32 : 0.26),
      "--ryo-focus": ac(dark ? 0.6 : 0.7),
      "--ryo-scroll-thumb": ink(dark ? 0.16 : 0.2),
      "--ryo-scroll-thumb-hover": ink(dark ? 0.28 : 0.32),
      "--ryo-display": dark
        ? `linear-gradient(180deg, ${n(0.13, 0.005)}, ${n(0.158, 0.005)})`
        : `linear-gradient(180deg, ${l(0.935)}, ${l(0.958)})`,
      "--ryo-play": v(c.accentHi, c.accentLo),
      "--ryo-primary": v(c.accentHi, c.accentLo),
      "--ryo-primary-hover": dark
        ? v(gold(0.92, 0.09), gold(0.78, 0.115))
        : v(amber(0.73, 0.13), amber(0.61, 0.14)),
      "--ryo-primary-ink": c.accentInk,
    },
  };
}
