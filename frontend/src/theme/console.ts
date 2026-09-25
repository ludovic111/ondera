/**
 * Console. A warm analogue desk: enamelled faceplate between walnut end cheeks,
 * brass for anything that is switched on, amber lamps behind the meters and the
 * counter. Dark is the control room at night (brown-black enamel, cream legends);
 * light is the same desk in daylight (cream enamel, umber legends). Both keep the
 * displays behind smoked amber glass, as the hardware would.
 *
 * Light falls from above as in skeuo.ts, but through tungsten: highlights are
 * warm cream, shade is umber, and glows are amber rather than white.
 */
import {
  eqGrid,
  physicalCanvasShadows,
  physicalGradients,
  physicalShadows,
  type Light,
} from "./materials";
import {
  alpha,
  families,
  ok,
  type ColorKey,
  type FillKey,
  type LineKey,
  type Mode,
  type ThemeSpec,
} from "./schema";

const FONT = "'Manrope', system-ui, -apple-system, sans-serif";
/** Pressed-steel panels, bakelite caps: small radii, round knobs. */
const RADIUS = {
  xs: 2,
  sm: 3,
  clip: 3,
  button: 3,
  md: 4,
  control: 5,
  lg: 6,
  glass: 6,
  window: 10,
};

/** Dark enamel: brown-black, hue 58. */
const en = (L: number, C = 0.012) => ok(L, C, 58);
/** Cream enamel: hue 85. */
const cr = (L: number, C = 0.022) => ok(L, C, 85);
/** Umber legend ink. */
const um = (L: number, C = 0.03) => ok(L, C, 62);
/** Brass, from polished (L .88) to tarnished (L .5). */
const brass = (L: number, C = 0.12) => ok(L, C, 80);
/** Amber lamp light behind the displays. */
const amber = (L: number, C = 0.14) => ok(L, C, 72);

/** Colour of the walnut cheeks, the same in both modes. */
const WALNUT = {
  dark: ok(0.27, 0.045, 50),
  mid: ok(0.34, 0.055, 52),
  light: ok(0.42, 0.06, 58),
  pore: ok(0.2, 0.035, 45),
};
/** Straight grain: open pores and lighter figure, running along the cheek. */
const grain = (deg: number) =>
  `repeating-linear-gradient(${deg}deg, transparent 0 7px, ${alpha(WALNUT.pore, 0.35)} 7px 8px, transparent 8px 19px, ${alpha(WALNUT.light, 0.25)} 19px 21px, transparent 21px 31px), repeating-linear-gradient(${deg - 4}deg, ${alpha(WALNUT.pore, 0.2)} 0 2px, transparent 2px 5px, ${alpha(WALNUT.light, 0.12)} 5px 6px, transparent 6px 11px)`;
const walnut = `${grain(92)}, linear-gradient(90deg, ${WALNUT.dark}, ${WALNUT.mid} 30%, ${WALNUT.light} 52%, ${WALNUT.mid} 74%, ${WALNUT.dark})`;

const darkColor: Record<ColorKey, string> = {
  desk: en(0.15, 0.01),
  wellDeep: ok(0.16, 0.022, 55),
  groove: en(0.185),
  grooveAlt: en(0.2),
  timelineEmpty: en(0.2),
  timeline: en(0.225),
  timelineAgent: ok(0.24, 0.028, 80),
  timelineSelected: en(0.255, 0.014),
  editor: en(0.23),
  ruler: en(0.25),
  materialCard: en(0.265),
  agentPanel: en(0.24),
  panel: en(0.27),
  logEntry: en(0.29),
  raised: en(0.31),
  controlFace: en(0.4),
  controlTop: en(0.39),
  controlBottom: en(0.3),
  pressedTop: en(0.21),
  pressedBottom: en(0.255),
  transportTop: en(0.32),
  transportBottom: en(0.27),
  segmentTop: en(0.43),
  segmentBottom: en(0.345),
  // Ivory fader caps and slider thumbs.
  thumbTop: cr(0.93, 0.03),
  thumbBottom: cr(0.72, 0.04),
  // Black bakelite knobs with a brass centre.
  knobHi: en(0.4, 0.01),
  knobLo: en(0.16, 0.01),
  knobBigHi: en(0.44, 0.01),
  knobBigLo: en(0.17, 0.01),
  knobInnerHi: brass(0.86, 0.1),
  knobInnerLo: brass(0.52, 0.1),
  capTop: cr(0.95, 0.03),
  capMid: cr(0.8, 0.035),
  capBottom: cr(0.62, 0.04),
  headerTop: en(0.285),
  headerBottom: en(0.265),
  headerSelectedTop: en(0.35, 0.016),
  headerSelectedBottom: en(0.315, 0.016),
  headerAgentTop: ok(0.31, 0.03, 80),
  headerAgentBottom: ok(0.28, 0.028, 80),
  insertTop: en(0.345),
  insertBottom: en(0.3),
  chipTop: en(0.35),
  chipBottom: en(0.285),
  trafficLight: en(0.42),
  inkBright: cr(0.965, 0.02),
  ink100: cr(0.925, 0.024),
  inkControl: cr(0.875, 0.028),
  inkDim: cr(0.8, 0.03),
  ink300: cr(0.74, 0.03),
  ink500: cr(0.65, 0.032),
  inkSeparator: en(0.46),
  inkKeyWhite: en(0.45),
  wellInk: amber(0.88, 0.13),
  wellInkDim: amber(0.74, 0.12),
  wellInkFaint: amber(0.6, 0.1),
  keyWhite: cr(0.92, 0.028),
  keyBlack: en(0.19),
  led: amber(0.84, 0.15),
  ledOff: en(0.235),
  ledGlow: "rgba(255,176,72,.75)",
  ledGlowSoft: "rgba(255,176,72,.3)",
  ledHot: ok(0.66, 0.2, 30),
  accent: brass(0.8, 0.125),
  accentHi: brass(0.9, 0.1),
  accentLo: brass(0.62, 0.12),
  accentInk: en(0.19, 0.03),
  noteTop: ok(0.84, 0.08, 168),
  noteBottom: ok(0.7, 0.09, 170),
  eqCurve: amber(0.84, 0.14),
  eqHandle: cr(0.95, 0.04),
  eqFill: amber(0.74, 0.14),
  ...families(0.78, 0.11),
  neutralDot: en(0.46),
  menu: en(0.305),
  menuHover: en(0.375, 0.016),
  menuSeparator: en(0.22),
  indicator: cr(0.94, 0.03),
  danger: ok(0.68, 0.19, 28),
  scrim: "rgba(14,9,4,.5)",
};

const lightColor: Record<ColorKey, string> = {
  desk: cr(0.74, 0.03),
  // The meters and the counter stay behind smoked amber glass in daylight.
  wellDeep: ok(0.2, 0.022, 55),
  groove: cr(0.8, 0.028),
  grooveAlt: cr(0.87, 0.024),
  timelineEmpty: cr(0.875, 0.024),
  timeline: cr(0.93, 0.02),
  timelineAgent: ok(0.92, 0.045, 92),
  timelineSelected: cr(0.955, 0.02),
  editor: cr(0.915, 0.022),
  ruler: cr(0.885, 0.026),
  materialCard: cr(0.905, 0.024),
  agentPanel: cr(0.905, 0.022),
  panel: cr(0.88, 0.026),
  logEntry: cr(0.925, 0.02),
  raised: cr(0.935, 0.02),
  controlFace: cr(0.96, 0.018),
  controlTop: cr(0.975, 0.016),
  controlBottom: cr(0.885, 0.026),
  pressedTop: cr(0.78, 0.03),
  pressedBottom: cr(0.84, 0.028),
  transportTop: cr(0.91, 0.024),
  transportBottom: cr(0.855, 0.028),
  segmentTop: cr(0.99, 0.012),
  segmentBottom: cr(0.925, 0.022),
  thumbTop: cr(0.995, 0.01),
  thumbBottom: cr(0.84, 0.03),
  // Ivory chicken-head knobs; the brass centre stays.
  knobHi: cr(0.995, 0.012),
  knobLo: cr(0.8, 0.034),
  knobBigHi: cr(1, 0.008),
  knobBigLo: cr(0.78, 0.036),
  knobInnerHi: brass(0.88, 0.1),
  knobInnerLo: brass(0.6, 0.11),
  capTop: cr(0.995, 0.01),
  capMid: cr(0.89, 0.026),
  capBottom: cr(0.78, 0.034),
  headerTop: cr(0.9, 0.024),
  headerBottom: cr(0.87, 0.026),
  headerSelectedTop: cr(0.96, 0.02),
  headerSelectedBottom: cr(0.92, 0.024),
  headerAgentTop: ok(0.93, 0.05, 92),
  headerAgentBottom: ok(0.89, 0.055, 90),
  insertTop: cr(0.95, 0.02),
  insertBottom: cr(0.89, 0.026),
  chipTop: cr(0.965, 0.018),
  chipBottom: cr(0.885, 0.026),
  trafficLight: cr(0.74, 0.03),
  inkBright: um(0.2, 0.03),
  ink100: um(0.25, 0.032),
  inkControl: um(0.3, 0.034),
  inkDim: um(0.36, 0.034),
  ink300: um(0.42, 0.034),
  ink500: um(0.48, 0.034),
  inkSeparator: cr(0.68, 0.03),
  inkKeyWhite: um(0.5, 0.02),
  wellInk: amber(0.88, 0.13),
  wellInkDim: amber(0.74, 0.12),
  wellInkFaint: amber(0.62, 0.1),
  keyWhite: cr(0.985, 0.012),
  keyBlack: um(0.24, 0.02),
  led: amber(0.84, 0.15),
  ledOff: ok(0.3, 0.02, 55),
  ledGlow: "rgba(255,170,60,.75)",
  ledGlowSoft: "rgba(255,170,60,.28)",
  ledHot: ok(0.64, 0.2, 30),
  // Brass darkens to bronze so it still reads on cream enamel.
  accent: brass(0.53, 0.115),
  accentHi: brass(0.8, 0.13),
  accentLo: brass(0.5, 0.11),
  accentInk: cr(0.99, 0.012),
  noteTop: ok(0.66, 0.1, 172),
  noteBottom: ok(0.54, 0.1, 175),
  eqCurve: amber(0.84, 0.14),
  eqHandle: cr(0.95, 0.04),
  eqFill: amber(0.74, 0.14),
  ...families(0.52, 0.13),
  neutralDot: cr(0.64, 0.03),
  menu: cr(0.95, 0.018),
  menuHover: cr(0.885, 0.028),
  menuSeparator: cr(0.83, 0.03),
  indicator: um(0.22, 0.03),
  danger: ok(0.53, 0.19, 28),
  scrim: "rgba(50,34,16,.3)",
};

/** Night shade is warm black; day shade is umber. */
const soot = (a: number) => `rgba(12,7,2,${a})`;
const umber = (a: number) => `rgba(66,44,20,${a})`;
const cream = (a: number) => `rgba(255,240,214,${a})`;
const lamp = (a: number) => `rgba(255,178,76,${a})`;

const darkFill: Record<FillKey, string> = {
  glass: "rgba(52,44,36,.7)",
  glassAgent: "rgba(58,50,36,.74)",
  glassCard: "rgba(62,54,40,.6)",
  logLive: "rgba(120,96,48,.2)",
  logChipLive: soot(0.25),
  browserHighlight: cream(0.07),
  cycleRuler: lamp(0.16),
  cycleLane: lamp(0.035),
  clipTitle: soot(0.24),
  rowShade: cream(0.03),
  blackKeyRow: soot(0.16),
  eqGridLine: lamp(0.06),
  eqZeroLine: lamp(0.14),
  velocity: cream(0.2),
  dragGhost: cream(0.12),
  pencilPreview: alpha(darkColor.accent, 0.25),
  cycleHandle: lamp(0.7),
  dropTarget: alpha(darkColor.accent, 0.12),
  stepCell: cream(0.06),
  hover: cream(0.05),
  fadeShade: soot(0.4),
  fadeHandle: cream(0.92),
  markerFlag: "rgba(22,16,10,.8)",
};

const darkLine: Record<LineKey, string> = {
  barLine: cream(0.075),
  beatLine: cream(0.025),
  rulerBar: cream(0.17),
  rulerTick: cream(0.075),
  cycleEdge: lamp(0.55),
  editorBar: cream(0.09),
  editorBeat: cream(0.03),
  waveform: cream(0.75),
  waveformMid: cream(0.33),
  midiNote: cream(0.8),
  clipName: cream(0.95),
  clipTitleBottom: soot(0.22),
  clipHighlight: cream(0.22),
  clipContact: soot(0.45),
  clipSelected: cream(0.9),
  cellDivider: cream(0.06),
  dividerDark: soot(0.5),
  laneTop: cream(0.02),
  laneBottom: soot(0.45),
  rulerBottom: soot(0.6),
  faderLine: soot(0.7),
  faderLineHi: cream(0.14),
  milled: soot(0.6),
  milledHi: cream(0.12),
  dragGhostEdge: cream(0.5),
  noteSelected: cream(0.95),
  noteHighlight: cream(0.4),
  splitGuide: alpha(darkColor.accent, 0.9),
  staff: cream(0.35),
  noteHead: cream(0.9),
  border: soot(0.55),
  borderStrong: soot(0.72),
  hairline: cream(0.05),
  fadeCurve: cream(0.85),
  // Chinagraph pencil on the scribble strip: orange, not brass.
  marker: ok(0.76, 0.15, 48),
  markerLane: ok(0.76, 0.15, 48, 0.3),
};

const lightFill: Record<FillKey, string> = {
  glass: "rgba(250,242,226,.8)",
  glassAgent: "rgba(248,240,214,.86)",
  glassCard: "rgba(250,244,228,.9)",
  logLive: ok(0.9, 0.05, 88, 0.7),
  logChipLive: umber(0.08),
  browserHighlight: umber(0.09),
  cycleRuler: alpha(lightColor.accent, 0.2),
  cycleLane: alpha(lightColor.accent, 0.045),
  clipTitle: umber(0.12),
  rowShade: umber(0.035),
  blackKeyRow: umber(0.07),
  eqGridLine: lamp(0.06),
  eqZeroLine: lamp(0.15),
  velocity: umber(0.32),
  dragGhost: umber(0.12),
  pencilPreview: alpha(lightColor.accent, 0.22),
  cycleHandle: alpha(lightColor.accent, 0.85),
  dropTarget: alpha(lightColor.accent, 0.12),
  stepCell: umber(0.07),
  hover: umber(0.06),
  fadeShade: umber(0.28),
  fadeHandle: "rgba(48,32,14,.85)",
  markerFlag: ok(0.97, 0.016, 85, 0.92),
};

const lightLine: Record<LineKey, string> = {
  barLine: umber(0.2),
  beatLine: umber(0.075),
  rulerBar: umber(0.42),
  rulerTick: umber(0.2),
  cycleEdge: alpha(lightColor.accent, 0.8),
  editorBar: umber(0.2),
  editorBeat: umber(0.075),
  waveform: "rgba(52,34,14,.78)",
  waveformMid: "rgba(52,34,14,.32)",
  midiNote: "rgba(52,34,14,.78)",
  clipName: "rgba(42,27,10,.93)",
  clipTitleBottom: umber(0.16),
  clipHighlight: cream(0.7),
  clipContact: umber(0.35),
  clipSelected: um(0.22),
  cellDivider: umber(0.12),
  dividerDark: umber(0.3),
  laneTop: cream(0.55),
  laneBottom: umber(0.16),
  rulerBottom: umber(0.32),
  faderLine: umber(0.55),
  faderLineHi: cream(0.9),
  milled: umber(0.45),
  milledHi: cream(0.9),
  dragGhostEdge: umber(0.5),
  noteSelected: um(0.2),
  noteHighlight: cream(0.45),
  splitGuide: alpha(lightColor.accent, 0.9),
  staff: umber(0.45),
  noteHead: um(0.24),
  border: umber(0.3),
  borderStrong: umber(0.48),
  hairline: umber(0.1),
  fadeCurve: "rgba(52,34,14,.8)",
  marker: ok(0.55, 0.16, 42),
  markerLane: ok(0.55, 0.16, 42, 0.35),
};

const LIGHTS: Record<Mode, Light> = {
  dark: {
    hi: (a) => cream(Number(Math.min(1, a * 1.1).toFixed(3))),
    lo: soot,
    glow: 1.1,
  },
  light: {
    hi: (a) => cream(Math.min(1, 0.55 + a * 1.4)),
    lo: (a) => umber(Number((a * 0.42).toFixed(3))),
    glow: 0.75,
  },
};

export function consoleTheme(mode: Mode): ThemeSpec {
  const dark = mode === "dark";
  const c = dark ? darkColor : lightColor;
  const fill = dark ? darkFill : lightFill;
  const line = dark ? darkLine : lightLine;
  const light = LIGHTS[mode];
  const gradient = physicalGradients(c);
  gradient.eqGrid = eqGrid(fill.eqGridLine, fill.eqZeroLine);
  const shadow = physicalShadows(c, light);

  // Lit keys are brass lamps: a warm dome with a hot centre.
  gradient.lit = `radial-gradient(circle at 50% 35%, ${c.accentHi}, ${c.accent} 55%, ${c.accentLo})`;
  gradient.sendKey = gradient.lit;
  // The transport is a brass-trimmed overbridge: enamel face, brass rail at the foot.
  const trim = dark ? brass(0.7, 0.1) : brass(0.62, 0.11);
  shadow.transport = `inset 0 1px 0 ${light.hi(0.07)}, inset 0 -2px 0 ${trim}, inset 0 -3px 0 ${light.lo(0.5)}, 0 2px 6px ${light.lo(0.45)}`;
  shadow.wellDeep = `inset 0 2px 6px ${soot(0.85)}, inset 0 0 0 1px ${soot(0.7)}, 0 0 0 1px ${alpha(trim, 0.45)}, 0 1px 0 ${light.hi(0.08)}`;
  if (!dark) {
    shadow.ledOff = `inset 0 1px 0 ${soot(0.5)}`;
    shadow.clipSelected = `0 0 0 1.5px ${c.ink100}`;
    gradient.titleBar = `linear-gradient(180deg, ${cr(0.91, 0.024)}, ${cr(0.875, 0.026)})`;
    gradient.panelHeader = `linear-gradient(180deg, ${cr(0.9, 0.024)}, ${cr(0.87, 0.026)})`;
  }

  return {
    scheme: mode,
    color: c,
    gradient,
    shadow,
    fill,
    line,
    blur: { glass: "8px", glassAgent: "10px", glassCard: "12px" },
    radius: RADIUS,
    // Heavy hardware: switches take a moment to seat, panels swing in with mass,
    // and things that land settle like a VU needle, with one small overshoot.
    motion: {
      fast: "90ms",
      base: "180ms",
      slow: "300ms",
      ease: "cubic-bezier(0.35, 0, 0.25, 1)",
      enter: "cubic-bezier(0.2, 0.8, 0.3, 1)",
      exit: "cubic-bezier(0.55, 0, 0.85, 0.35)",
      settle: "cubic-bezier(0.34, 1.45, 0.55, 1)",
    },
    canvasShadow: physicalCanvasShadows(c, light),
    clipMix: dark
      ? { faceTop: 64, faceBottom: 50 }
      : { faceTop: 60, faceBottom: 76 },
    fontUi: FONT,
    // Engraved legends are set wide.
    capsTracking: "0.12em",
    vars: {
      "--display-glow": dark ? "0.28" : "0.22",
      "--ring-glow": alpha(c.led, dark ? 0.5 : 0.35),
      "--console-frame": walnut,
      "--console-cheek-shadow": `inset 0 0 0 1px ${alpha(WALNUT.pore, 0.9)}, inset 0 1px 0 ${alpha(WALNUT.light, 0.6)}`,
      "--console-title": `${grain(2)}, linear-gradient(180deg, ${WALNUT.mid}, ${WALNUT.dark})`,
      "--console-title-ink": cr(0.93, 0.03),
      "--console-title-ink-dim": cr(0.8, 0.035),
      "--console-title-hover": cream(0.14),
      "--console-engrave": dark
        ? `0 -1px 0 ${soot(0.7)}`
        : `0 1px 0 ${cream(0.85)}`,
      "--console-lcd": `radial-gradient(ellipse at 50% 0%, ${lamp(0.1)}, transparent 70%), ${c.wellDeep}`,
      "--console-lcd-glow": `0 0 6px ${lamp(0.55)}`,
      "--console-brass": `linear-gradient(180deg, ${brass(0.9, 0.09)}, ${brass(0.7, 0.12)} 50%, ${brass(0.56, 0.11)})`,
    },
  };
}
