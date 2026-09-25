/**
 * Neon. A synth at midnight: smoked violet glass lit from inside by two gases,
 * magenta for what you act on (the accent, the playhead, lit keys) and cyan for what
 * the machine tells you (meters, displays, notes). Light does not fall from above:
 * surfaces are dark and matte, and anything alive glows, so depth reads as emission
 * rather than relief. The glow is kept to edges and lit things; text never glows.
 *
 * Light mode is the same instrument at dawn: lilac haze, violet ink, the tubes still
 * on behind dark glass displays, with the glow turned down to a tint.
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
  white,
  type ColorKey,
  type FillKey,
  type LineKey,
  type Mode,
  type ThemeSpec,
} from "./schema";

const FONT = "'Manrope', system-ui, -apple-system, sans-serif";
const RADIUS = {
  xs: 2,
  sm: 4,
  clip: 5,
  button: 6,
  md: 7,
  control: 8,
  lg: 10,
  glass: 12,
  window: 12,
};

/** Night violet: hue 290. */
const nv = (L: number, C = 0.04) => ok(L, C, 290);
/** Dawn lilac: hue 305. */
const lv = (L: number, C = 0.02) => ok(L, C, 305);
const MAGENTA = 350;
const CYAN = 200;
const cyan = (L: number, C = 0.12) => ok(L, C, CYAN);

const darkColor: Record<ColorKey, string> = {
  desk: nv(0.13, 0.045),
  wellDeep: ok(0.1, 0.035, 285),
  groove: nv(0.155, 0.04),
  grooveAlt: nv(0.17, 0.04),
  timelineEmpty: nv(0.16, 0.04),
  timeline: nv(0.18, 0.04),
  timelineAgent: ok(0.205, 0.05, 330),
  timelineSelected: nv(0.21, 0.05),
  editor: nv(0.175, 0.04),
  ruler: nv(0.2, 0.045),
  materialCard: nv(0.22, 0.045),
  agentPanel: nv(0.19, 0.042),
  panel: nv(0.205, 0.045),
  logEntry: nv(0.23, 0.045),
  raised: nv(0.25, 0.05),
  controlFace: nv(0.34, 0.06),
  controlTop: nv(0.3, 0.055),
  controlBottom: nv(0.235, 0.05),
  pressedTop: nv(0.13, 0.045),
  pressedBottom: nv(0.17, 0.045),
  transportTop: nv(0.235, 0.05),
  transportBottom: nv(0.185, 0.045),
  segmentTop: ok(0.36, 0.1, 305),
  segmentBottom: ok(0.28, 0.09, 300),
  thumbTop: cyan(0.93, 0.05),
  thumbBottom: cyan(0.72, 0.1),
  knobHi: nv(0.36, 0.055),
  knobLo: nv(0.15, 0.045),
  knobBigHi: nv(0.4, 0.06),
  knobBigLo: nv(0.16, 0.045),
  knobInnerHi: nv(0.26, 0.05),
  knobInnerLo: nv(0.17, 0.045),
  capTop: cyan(0.92, 0.05),
  capMid: cyan(0.72, 0.09),
  capBottom: ok(0.5, 0.1, 230),
  headerTop: nv(0.22, 0.045),
  headerBottom: nv(0.2, 0.045),
  headerSelectedTop: ok(0.28, 0.07, 305),
  headerSelectedBottom: ok(0.245, 0.065, 305),
  headerAgentTop: ok(0.26, 0.07, 335),
  headerAgentBottom: ok(0.225, 0.06, 335),
  insertTop: nv(0.28, 0.05),
  insertBottom: nv(0.235, 0.05),
  chipTop: nv(0.29, 0.055),
  chipBottom: nv(0.23, 0.05),
  trafficLight: nv(0.36, 0.05),
  inkBright: ok(0.97, 0.015, 300),
  ink100: ok(0.93, 0.02, 300),
  inkControl: ok(0.88, 0.03, 300),
  inkDim: ok(0.81, 0.04, 300),
  ink300: ok(0.75, 0.045, 298),
  ink500: ok(0.65, 0.05, 295),
  inkSeparator: nv(0.42, 0.06),
  inkKeyWhite: nv(0.45, 0.04),
  wellInk: cyan(0.92, 0.1),
  wellInkDim: cyan(0.77, 0.11),
  wellInkFaint: cyan(0.63, 0.1),
  keyWhite: ok(0.9, 0.02, 300),
  keyBlack: nv(0.16, 0.04),
  led: cyan(0.85, 0.14),
  ledOff: nv(0.22, 0.05),
  ledGlow: "rgba(70,235,255,.8)",
  ledGlowSoft: "rgba(70,235,255,.3)",
  ledHot: ok(0.72, 0.24, MAGENTA),
  accent: ok(0.74, 0.2, 345),
  accentHi: ok(0.86, 0.13, 345),
  accentLo: ok(0.6, 0.23, 350),
  accentInk: ok(0.16, 0.05, 340),
  noteTop: cyan(0.85, 0.12),
  noteBottom: ok(0.7, 0.13, 215),
  eqCurve: cyan(0.86, 0.13),
  eqHandle: cyan(0.96, 0.05),
  eqFill: cyan(0.74, 0.14),
  ...families(0.8, 0.13),
  neutralDot: nv(0.45, 0.05),
  menu: nv(0.25, 0.05),
  menuHover: ok(0.33, 0.08, 305),
  menuSeparator: nv(0.18, 0.045),
  indicator: cyan(0.92, 0.09),
  danger: ok(0.7, 0.2, 22),
  scrim: "rgba(8,2,22,.6)",
};

const lightColor: Record<ColorKey, string> = {
  desk: lv(0.86, 0.04),
  // The displays stay dark glass with the tubes on behind them.
  wellDeep: ok(0.19, 0.06, 290),
  groove: lv(0.87, 0.035),
  grooveAlt: lv(0.93, 0.02),
  timelineEmpty: lv(0.93, 0.022),
  timeline: lv(0.965, 0.012),
  timelineAgent: ok(0.945, 0.03, 345),
  timelineSelected: lv(0.93, 0.03),
  editor: lv(0.96, 0.014),
  ruler: lv(0.935, 0.022),
  materialCard: lv(0.975, 0.01),
  agentPanel: lv(0.955, 0.016),
  panel: lv(0.945, 0.018),
  logEntry: lv(0.97, 0.012),
  raised: lv(0.98, 0.01),
  controlFace: lv(0.85, 0.04),
  controlTop: lv(0.995, 0.006),
  controlBottom: lv(0.91, 0.028),
  pressedTop: lv(0.84, 0.045),
  pressedBottom: lv(0.89, 0.035),
  transportTop: lv(0.97, 0.014),
  transportBottom: lv(0.915, 0.028),
  segmentTop: ok(0.97, 0.025, 330),
  segmentBottom: ok(0.885, 0.055, 325),
  thumbTop: lv(1, 0),
  thumbBottom: lv(0.86, 0.04),
  knobHi: lv(1, 0),
  knobLo: lv(0.84, 0.045),
  knobBigHi: lv(1, 0),
  knobBigLo: lv(0.83, 0.045),
  knobInnerHi: lv(0.95, 0.02),
  knobInnerLo: lv(0.88, 0.035),
  capTop: lv(1, 0),
  capMid: lv(0.92, 0.025),
  capBottom: lv(0.82, 0.045),
  headerTop: lv(0.955, 0.016),
  headerBottom: lv(0.935, 0.022),
  headerSelectedTop: ok(0.955, 0.035, 320),
  headerSelectedBottom: ok(0.915, 0.05, 320),
  headerAgentTop: ok(0.96, 0.035, 345),
  headerAgentBottom: ok(0.925, 0.045, 345),
  insertTop: lv(0.985, 0.008),
  insertBottom: lv(0.93, 0.024),
  chipTop: lv(0.99, 0.006),
  chipBottom: lv(0.92, 0.026),
  trafficLight: lv(0.8, 0.04),
  inkBright: ok(0.2, 0.07, 295),
  ink100: ok(0.24, 0.07, 295),
  inkControl: ok(0.3, 0.07, 295),
  inkDim: ok(0.36, 0.065, 295),
  ink300: ok(0.42, 0.065, 295),
  ink500: ok(0.5, 0.06, 295),
  inkSeparator: lv(0.74, 0.04),
  inkKeyWhite: ok(0.5, 0.04, 295),
  wellInk: cyan(0.92, 0.1),
  wellInkDim: cyan(0.77, 0.11),
  wellInkFaint: cyan(0.64, 0.1),
  keyWhite: lv(1, 0),
  keyBlack: ok(0.28, 0.05, 295),
  led: cyan(0.85, 0.14),
  ledOff: ok(0.28, 0.05, 290),
  ledGlow: "rgba(60,225,255,.7)",
  ledGlowSoft: "rgba(60,225,255,.25)",
  ledHot: ok(0.72, 0.22, MAGENTA),
  accent: ok(0.55, 0.22, 350),
  accentHi: ok(0.8, 0.14, 345),
  accentLo: ok(0.5, 0.21, 352),
  accentInk: "#ffffff",
  noteTop: ok(0.66, 0.16, 300),
  noteBottom: ok(0.54, 0.18, 295),
  eqCurve: cyan(0.86, 0.13),
  eqHandle: cyan(0.96, 0.05),
  eqFill: cyan(0.74, 0.14),
  ...families(0.52, 0.15),
  neutralDot: lv(0.7, 0.04),
  menu: lv(0.985, 0.008),
  menuHover: ok(0.93, 0.045, 320),
  menuSeparator: lv(0.88, 0.03),
  indicator: ok(0.28, 0.08, 295),
  danger: ok(0.54, 0.21, 25),
  scrim: "rgba(60,20,90,.25)",
};

/** Night shade is violet-black; the light it emits is violet-white. */
const deep = (a: number) => `rgba(6,0,20,${a})`;
const haze = (a: number) => `rgba(206,190,255,${a})`;
/** Dawn shade is plum. */
const plum = (a: number) => `rgba(58,24,92,${a})`;
const tube = (a: number) => `rgba(70,235,255,${a})`;

const darkFill: Record<FillKey, string> = {
  glass: "rgba(34,22,58,.78)",
  glassAgent: "rgba(44,20,60,.82)",
  glassCard: "rgba(46,24,66,.86)",
  logLive: alpha(darkColor.accent, 0.1),
  logChipLive: deep(0.35),
  browserHighlight: haze(0.1),
  cycleRuler: tube(0.18),
  cycleLane: tube(0.035),
  clipTitle: deep(0.25),
  rowShade: haze(0.03),
  blackKeyRow: deep(0.2),
  eqGridLine: tube(0.07),
  eqZeroLine: tube(0.18),
  velocity: haze(0.3),
  dragGhost: haze(0.14),
  pencilPreview: alpha(darkColor.accent, 0.25),
  cycleHandle: tube(0.85),
  dropTarget: alpha(darkColor.accent, 0.12),
  stepCell: haze(0.07),
  hover: haze(0.07),
  fadeShade: deep(0.4),
  fadeHandle: white(0.92),
  markerFlag: "rgba(14,6,30,.8)",
};

const darkLine: Record<LineKey, string> = {
  barLine: haze(0.09),
  beatLine: haze(0.03),
  rulerBar: haze(0.22),
  rulerTick: haze(0.1),
  cycleEdge: tube(0.8),
  editorBar: haze(0.1),
  editorBeat: haze(0.035),
  waveform: white(0.8),
  waveformMid: white(0.32),
  midiNote: white(0.82),
  clipName: white(0.95),
  clipTitleBottom: deep(0.25),
  clipHighlight: haze(0.3),
  clipContact: deep(0.5),
  clipSelected: "#ffffff",
  cellDivider: haze(0.07),
  dividerDark: deep(0.55),
  laneTop: haze(0.03),
  laneBottom: deep(0.45),
  rulerBottom: deep(0.6),
  faderLine: deep(0.7),
  faderLineHi: haze(0.2),
  milled: deep(0.6),
  milledHi: haze(0.14),
  dragGhostEdge: haze(0.6),
  noteSelected: "#ffffff",
  noteHighlight: white(0.45),
  splitGuide: alpha(darkColor.accent, 0.95),
  staff: haze(0.4),
  noteHead: white(0.92),
  border: deep(0.7),
  borderStrong: haze(0.22),
  hairline: haze(0.06),
  fadeCurve: white(0.85),
  marker: ok(0.86, 0.15, 95),
  markerLane: ok(0.86, 0.15, 95, 0.3),
};

const lightFill: Record<FillKey, string> = {
  glass: "rgba(250,244,255,.82)",
  glassAgent: "rgba(255,240,250,.88)",
  glassCard: "rgba(255,244,252,.92)",
  logLive: alpha(lightColor.accent, 0.07),
  logChipLive: plum(0.08),
  browserHighlight: plum(0.08),
  cycleRuler: alpha(lightColor.accent, 0.16),
  cycleLane: alpha(lightColor.accent, 0.035),
  clipTitle: white(0.3),
  rowShade: plum(0.03),
  blackKeyRow: plum(0.055),
  eqGridLine: tube(0.07),
  eqZeroLine: tube(0.18),
  velocity: plum(0.35),
  dragGhost: plum(0.12),
  pencilPreview: alpha(lightColor.accent, 0.22),
  cycleHandle: alpha(lightColor.accent, 0.85),
  dropTarget: alpha(lightColor.accent, 0.1),
  stepCell: plum(0.07),
  hover: plum(0.06),
  fadeShade: plum(0.22),
  fadeHandle: ok(0.26, 0.08, 295),
  markerFlag: "rgba(255,250,255,.9)",
};

const lightLine: Record<LineKey, string> = {
  barLine: plum(0.16),
  beatLine: plum(0.06),
  rulerBar: plum(0.36),
  rulerTick: plum(0.18),
  cycleEdge: alpha(lightColor.accent, 0.85),
  editorBar: plum(0.16),
  editorBeat: plum(0.06),
  waveform: "rgba(40,16,70,.78)",
  waveformMid: "rgba(40,16,70,.3)",
  midiNote: "rgba(40,16,70,.8)",
  clipName: "rgba(34,12,60,.92)",
  clipTitleBottom: plum(0.14),
  clipHighlight: white(0.8),
  clipContact: plum(0.3),
  clipSelected: ok(0.3, 0.12, 300),
  cellDivider: plum(0.1),
  dividerDark: lv(0.84, 0.04),
  laneTop: white(0.7),
  laneBottom: plum(0.11),
  rulerBottom: lv(0.8, 0.045),
  faderLine: plum(0.55),
  faderLineHi: white(0.95),
  milled: plum(0.45),
  milledHi: white(0.95),
  dragGhostEdge: plum(0.5),
  noteSelected: ok(0.25, 0.1, 300),
  noteHighlight: white(0.55),
  splitGuide: alpha(lightColor.accent, 0.9),
  staff: plum(0.4),
  noteHead: ok(0.3, 0.07, 295),
  border: lv(0.8, 0.045),
  borderStrong: lv(0.68, 0.06),
  hairline: plum(0.08),
  fadeCurve: "rgba(40,16,70,.8)",
  marker: ok(0.58, 0.16, 55),
  markerLane: ok(0.58, 0.16, 55, 0.35),
};

const LIGHTS: Record<Mode, Light> = {
  dark: {
    // Emitted, not reflected: faint violet edges, deep shade, strong glow.
    hi: (a) => haze(Number(Math.min(1, a * 0.9).toFixed(3))),
    lo: deep,
    glow: 1.35,
  },
  light: {
    hi: (a) => white(Math.min(1, 0.6 + a * 1.4)),
    lo: (a) => plum(Number((a * 0.38).toFixed(3))),
    glow: 0.8,
  },
};

export function neon(mode: Mode): ThemeSpec {
  const dark = mode === "dark";
  const c = dark ? darkColor : lightColor;
  const fill = dark ? darkFill : lightFill;
  const line = dark ? darkLine : lightLine;
  const light = LIGHTS[mode];
  const gradient = physicalGradients(c);
  gradient.eqGrid = eqGrid(fill.eqGridLine, fill.eqZeroLine);
  const shadow = physicalShadows(c, light);
  const ac = (a: number) => alpha(c.accent, Math.min(1, a * light.glow));
  const cy = (a: number) => tube(Math.min(1, a * light.glow));

  // Lit keys are a tube seen end-on: hot core, magenta body.
  gradient.lit = `radial-gradient(ellipse at 50% 40%, ${c.accentHi}, ${c.accent} 50%, ${c.accentLo})`;
  gradient.sendKey = gradient.lit;
  // The selected segment is lit from its lower edge.
  gradient.segment = `radial-gradient(ellipse at 50% 120%, ${ac(0.45)}, transparent 70%), linear-gradient(180deg, ${c.segmentTop}, ${c.segmentBottom})`;
  gradient.dialog = dark
    ? `radial-gradient(ellipse at 50% -30%, ${alpha(c.accent, 0.12)}, transparent 60%), linear-gradient(180deg, ${nv(0.235, 0.05)}, ${nv(0.2, 0.045)})`
    : `radial-gradient(ellipse at 50% -30%, ${alpha(c.accent, 0.08)}, transparent 60%), linear-gradient(180deg, ${lv(0.99, 0.006)}, ${lv(0.965, 0.014)})`;

  if (dark) {
    shadow.raised = `inset 0 1px 0 ${haze(0.1)}, inset 0 0 0 1px ${haze(0.07)}, 0 1px 2px ${deep(0.6)}, 0 3px 6px ${deep(0.3)}`;
    shadow.segment = `inset 0 1px 0 ${haze(0.18)}, inset 0 0 0 1px ${ac(0.55)}, 0 0 10px ${ac(0.3)}`;
    shadow.wellDeep = `inset 0 2px 6px ${deep(0.9)}, inset 0 0 0 1px ${cy(0.16)}, 0 0 12px ${cy(0.08)}`;
    shadow.menu = `inset 0 1px 0 ${haze(0.12)}, 0 0 0 1px ${haze(0.16)}, 0 0 24px ${ac(0.12)}, 0 12px 32px ${deep(0.7)}`;
    shadow.dialog = `inset 0 1px 0 ${haze(0.14)}, 0 0 0 1px ${haze(0.18)}, 0 0 40px ${ac(0.14)}, 0 24px 70px ${deep(0.75)}`;
    shadow.clipSelected = `0 0 0 1.5px #ffffff, 0 0 10px ${cy(0.4)}`;
    shadow.faderCap = `inset 0 1px 0 ${white(0.6)}, inset 0 -1px 0 ${deep(0.4)}, 0 0 8px ${cy(0.45)}, 0 3px 6px ${deep(0.6)}`;
    shadow.thumb = `inset 0 1px 0 ${white(0.5)}, 0 0 6px ${cy(0.5)}, 0 1px 3px ${deep(0.6)}`;
  } else {
    shadow.segment = `inset 0 1px 0 ${white(0.9)}, inset 0 0 0 1px ${ac(0.45)}, 0 0 8px ${ac(0.2)}`;
    shadow.wellDeep = `inset 0 2px 5px ${deep(0.75)}, inset 0 0 0 1px ${deep(0.6)}, 0 1px 0 ${white(0.85)}, 0 0 10px ${cy(0.15)}`;
    shadow.ledOff = `inset 0 1px 0 ${deep(0.5)}`;
    shadow.clipSelected = `0 0 0 1.5px ${c.ink100}`;
    shadow.menu = `inset 0 1px 0 #fff, 0 0 0 1px ${lv(0.8, 0.05)}, 0 10px 28px ${plum(0.2)}`;
    shadow.dialog = `inset 0 1px 0 #fff, 0 0 0 1px ${lv(0.75, 0.06)}, 0 0 30px ${ac(0.1)}, 0 20px 60px ${plum(0.28)}`;
  }

  const canvasShadow = physicalCanvasShadows(c, light);
  // Selected notes glow cyan, like the rest of what the machine draws.
  canvasShadow.noteSelected = [{ blur: 8, offsetY: 0, color: cy(0.55) }];

  return {
    scheme: mode,
    color: c,
    gradient,
    shadow,
    fill,
    line,
    blur: { glass: "14px", glassAgent: "18px", glassCard: "20px" },
    radius: RADIUS,
    // Gas discharge: things strike quickly and then bloom; nothing bounces hard.
    motion: {
      fast: "75ms",
      base: "170ms",
      slow: "280ms",
      ease: "cubic-bezier(0.3, 0, 0.1, 1)",
      enter: "cubic-bezier(0.05, 0.7, 0.1, 1)",
      exit: "cubic-bezier(0.4, 0, 0.9, 0.3)",
      settle: "cubic-bezier(0.2, 1.2, 0.4, 1)",
    },
    canvasShadow,
    clipMix: dark
      ? { faceTop: 60, faceBottom: 42 }
      : { faceTop: 48, faceBottom: 70 },
    fontUi: FONT,
    capsTracking: "0.1em",
    vars: {
      "--display-glow": dark ? "0.4" : "0.28",
      "--ring-glow": ac(dark ? 0.75 : 0.35),
      // A thin tube along the foot of the menu bar and of the transport.
      "--neon-rule": `linear-gradient(90deg, transparent, ${ac(0.9)} 18%, ${cy(0.9)} 82%, transparent)`,
      "--neon-rule-glow": `0 0 8px ${ac(0.5)}, 0 0 14px ${cy(0.25)}`,
      "--neon-frame": dark
        ? `radial-gradient(ellipse at 50% 120%, ${alpha(c.accent, 0.35)}, transparent 55%), radial-gradient(ellipse at 100% 0%, ${tube(0.18)}, transparent 50%), ${c.desk}`
        : `radial-gradient(ellipse at 50% 120%, ${alpha(c.accent, 0.25)}, transparent 55%), radial-gradient(ellipse at 100% 0%, ${tube(0.25)}, transparent 50%), ${c.desk}`,
      "--neon-lcd": `radial-gradient(ellipse at 50% 130%, ${tube(0.14)}, transparent 70%), ${c.wellDeep}`,
      "--neon-lcd-glow": `0 0 6px ${tube(dark ? 0.7 : 0.55)}`,
      "--neon-tab-glow": `0 0 8px ${ac(0.6)}`,
    },
  };
}
