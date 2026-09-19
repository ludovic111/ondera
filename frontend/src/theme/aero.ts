/**
 * Frutiger Aero. Sky, water, grass and glass: a glossy frame around calm opaque
 * work surfaces. Every raised control is a lens: bright upper reflection, a hard
 * waist at 48 %, a luminous lower rim. Reading surfaces (lanes, lists, the
 * conversation) stay opaque so content never competes with the chrome.
 *
 * Light is a daylight sky over pearl; dark is the same glass at night under an
 * aurora, where light comes from inside the controls instead of from above.
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
  black,
  ok,
  white,
  type ColorKey,
  type FillKey,
  type LineKey,
  type Mode,
  type ThemeSpec,
} from "./schema";

const FONT =
  "'Segoe UI', 'Lucida Grande', 'Frutiger', 'Trebuchet MS', 'Manrope', sans-serif";
const RADIUS = {
  xs: 2,
  sm: 4,
  clip: 6,
  button: 6,
  md: 7,
  control: 9,
  lg: 10,
  glass: 12,
  window: 12,
};

/** Pearl-to-sky neutral: hue 225, chroma rises as surfaces get darker. */
const sky = (L: number, C = 0.018) => ok(L, C, 225);
/** Night water: hue 238. */
const sea = (L: number, C = 0.035) => ok(L, C, 238);

const lightColor: Record<ColorKey, string> = {
  desk: "#7bbbd6",
  wellDeep: "#0b2230",
  groove: sky(0.9, 0.022),
  grooveAlt: sky(0.975, 0.008),
  timelineEmpty: sky(0.935, 0.012),
  timeline: sky(0.975, 0.006),
  timelineAgent: ok(0.955, 0.03, 165),
  timelineSelected: sky(0.925, 0.03),
  editor: sky(0.96, 0.01),
  ruler: sky(0.94, 0.016),
  materialCard: sky(0.985, 0.006),
  agentPanel: sky(0.975, 0.008),
  panel: sky(0.945, 0.015),
  logEntry: sky(0.96, 0.012),
  raised: sky(0.915, 0.025),
  controlFace: sky(0.76, 0.04),
  controlTop: "#ffffff",
  controlBottom: sky(0.88, 0.03),
  pressedTop: sky(0.83, 0.045),
  pressedBottom: sky(0.94, 0.025),
  transportTop: "#f6fdff",
  transportBottom: "#bad8e8",
  segmentTop: ok(0.95, 0.04, 215),
  segmentBottom: ok(0.81, 0.075, 225),
  thumbTop: "#ffffff",
  thumbBottom: sky(0.8, 0.035),
  knobHi: "#ffffff",
  knobLo: sky(0.76, 0.04),
  knobBigHi: "#ffffff",
  knobBigLo: sky(0.77, 0.04),
  knobInnerHi: sky(0.95, 0.015),
  knobInnerLo: sky(0.83, 0.03),
  capTop: "#ffffff",
  capMid: sky(0.91, 0.02),
  capBottom: sky(0.75, 0.04),
  headerTop: sky(0.985, 0.006),
  headerBottom: sky(0.925, 0.018),
  headerSelectedTop: ok(0.965, 0.03, 220),
  headerSelectedBottom: ok(0.86, 0.055, 228),
  headerAgentTop: ok(0.975, 0.03, 160),
  headerAgentBottom: ok(0.895, 0.045, 165),
  insertTop: sky(0.985, 0.006),
  insertBottom: sky(0.88, 0.03),
  chipTop: "#ffffff",
  chipBottom: sky(0.89, 0.028),
  trafficLight: sky(0.82, 0.03),
  inkBright: ok(0.3, 0.06, 232),
  ink100: ok(0.31, 0.04, 230),
  inkControl: ok(0.36, 0.05, 230),
  inkDim: ok(0.4, 0.045, 228),
  ink300: ok(0.44, 0.045, 228),
  ink500: ok(0.5, 0.04, 228),
  inkSeparator: sky(0.71, 0.04),
  inkKeyWhite: ok(0.46, 0.04, 228),
  wellInk: "#d6fbb4",
  wellInkDim: "#9fd07f",
  wellInkFaint: "#86ab84",
  keyWhite: "#fcfefe",
  keyBlack: ok(0.35, 0.035, 228),
  led: ok(0.78, 0.2, 135),
  ledOff: "#1b3a3a",
  ledGlow: "rgba(140,230,80,.7)",
  ledGlowSoft: "rgba(140,230,80,.25)",
  ledHot: ok(0.85, 0.17, 95),
  accent: ok(0.5, 0.125, 240),
  accentHi: ok(0.9, 0.07, 215),
  accentLo: ok(0.66, 0.12, 228),
  accentInk: "#102908",
  noteTop: ok(0.74, 0.11, 235),
  noteBottom: ok(0.55, 0.14, 245),
  eqCurve: ok(0.86, 0.17, 140),
  eqHandle: "#efffd8",
  eqFill: ok(0.78, 0.2, 135),
  neutralDot: sky(0.7, 0.035),
  menu: sky(0.985, 0.006),
  menuHover: ok(0.92, 0.04, 222),
  menuSeparator: sky(0.85, 0.028),
  indicator: ok(0.32, 0.07, 235),
  danger: ok(0.56, 0.2, 27),
  scrim: "rgba(12,44,70,.3)",
};

const darkColor: Record<ColorKey, string> = {
  desk: sea(0.16, 0.04),
  wellDeep: "#020d16",
  groove: sea(0.15),
  grooveAlt: sea(0.175),
  timelineEmpty: sea(0.18, 0.028),
  timeline: sea(0.21, 0.028),
  timelineAgent: ok(0.235, 0.04, 185),
  timelineSelected: sea(0.255, 0.04),
  editor: sea(0.215, 0.028),
  ruler: sea(0.245, 0.035),
  materialCard: sea(0.275),
  agentPanel: sea(0.225, 0.03),
  panel: sea(0.25, 0.034),
  logEntry: sea(0.28),
  raised: sea(0.31, 0.04),
  controlFace: sea(0.42, 0.045),
  controlTop: sea(0.5, 0.05),
  controlBottom: sea(0.28, 0.045),
  pressedTop: sea(0.155),
  pressedBottom: sea(0.23, 0.04),
  transportTop: sea(0.36, 0.05),
  transportBottom: sea(0.2, 0.04),
  segmentTop: ok(0.62, 0.11, 228),
  segmentBottom: ok(0.42, 0.12, 240),
  thumbTop: sea(0.93, 0.02),
  thumbBottom: sea(0.58, 0.05),
  knobHi: sea(0.7, 0.04),
  knobLo: sea(0.2, 0.04),
  knobBigHi: sea(0.74, 0.04),
  knobBigLo: sea(0.2, 0.04),
  knobInnerHi: sea(0.36, 0.045),
  knobInnerLo: sea(0.2, 0.04),
  capTop: sea(0.92, 0.02),
  capMid: sea(0.62, 0.045),
  capBottom: sea(0.38, 0.05),
  headerTop: sea(0.275, 0.036),
  headerBottom: sea(0.235, 0.034),
  headerSelectedTop: ok(0.37, 0.07, 236),
  headerSelectedBottom: ok(0.29, 0.065, 240),
  headerAgentTop: ok(0.32, 0.055, 180),
  headerAgentBottom: ok(0.255, 0.05, 185),
  insertTop: sea(0.345, 0.042),
  insertBottom: sea(0.26, 0.04),
  chipTop: sea(0.38, 0.045),
  chipBottom: sea(0.27, 0.04),
  trafficLight: sea(0.4, 0.04),
  inkBright: ok(0.975, 0.015, 215),
  ink100: ok(0.935, 0.02, 222),
  inkControl: ok(0.9, 0.025, 222),
  inkDim: ok(0.82, 0.03, 225),
  ink300: ok(0.77, 0.035, 228),
  ink500: ok(0.66, 0.04, 230),
  inkSeparator: sea(0.43, 0.045),
  inkKeyWhite: sea(0.45, 0.03),
  wellInk: "#c8f6ff",
  wellInkDim: "#7fc3d8",
  wellInkFaint: "#5e93a8",
  keyWhite: sea(0.9, 0.015),
  keyBlack: sea(0.17, 0.03),
  led: ok(0.86, 0.21, 135),
  ledOff: "#0c2a30",
  ledGlow: "rgba(150,245,90,.85)",
  ledGlowSoft: "rgba(150,245,90,.35)",
  ledHot: ok(0.88, 0.17, 95),
  accent: ok(0.82, 0.12, 215),
  accentHi: ok(0.93, 0.07, 205),
  accentLo: ok(0.62, 0.14, 238),
  accentInk: "#0c2406",
  noteTop: ok(0.83, 0.11, 215),
  noteBottom: ok(0.6, 0.15, 240),
  eqCurve: ok(0.86, 0.2, 138),
  eqHandle: "#efffd8",
  eqFill: ok(0.8, 0.2, 135),
  neutralDot: sea(0.5, 0.04),
  menu: sea(0.29, 0.04),
  menuHover: ok(0.4, 0.085, 236),
  menuSeparator: sea(0.2, 0.035),
  indicator: ok(0.95, 0.05, 205),
  danger: ok(0.7, 0.18, 25),
  scrim: "rgba(1,8,18,.55)",
};

/** Shade in daylight is deep water blue; at night it is near black. */
const water = (a: number) => `rgba(28,70,94,${a})`;
const glowLine = (a: number) => `rgba(170,220,255,${a})`;

const lightFill: Record<FillKey, string> = {
  glass: "rgba(240,251,255,.84)",
  glassAgent: "rgba(228,246,255,.9)",
  glassCard: "rgba(239,250,255,.94)",
  logLive: "#e0f2fa",
  logChipLive: "#c5e7f4",
  browserHighlight: "rgba(33,123,168,.13)",
  cycleRuler: "rgba(64,155,71,.2)",
  cycleLane: "rgba(64,155,71,.04)",
  clipTitle: "rgba(255,255,255,.3)",
  rowShade: water(0.035),
  blackKeyRow: water(0.06),
  eqGridLine: "rgba(160,255,190,.08)",
  eqZeroLine: "rgba(160,255,190,.2)",
  velocity: "rgba(23,95,135,.45)",
  dragGhost: water(0.14),
  pencilPreview: ok(0.5, 0.125, 240, 0.22),
  cycleHandle: "rgba(64,155,71,.8)",
  dropTarget: ok(0.5, 0.125, 240, 0.1),
  stepCell: water(0.08),
  hover: "rgba(33,123,168,.09)",
};

const darkFill: Record<FillKey, string> = {
  glass: "rgba(18,44,72,.72)",
  glassAgent: "rgba(16,52,78,.78)",
  glassCard: "rgba(20,50,80,.82)",
  logLive: "rgba(60,190,230,.12)",
  logChipLive: "rgba(0,8,20,.4)",
  browserHighlight: glowLine(0.12),
  cycleRuler: "rgba(150,245,90,.2)",
  cycleLane: "rgba(150,245,90,.035)",
  clipTitle: black(0.2),
  rowShade: glowLine(0.03),
  blackKeyRow: black(0.18),
  eqGridLine: "rgba(140,230,255,.07)",
  eqZeroLine: "rgba(140,230,255,.18)",
  velocity: glowLine(0.3),
  dragGhost: glowLine(0.14),
  pencilPreview: ok(0.82, 0.12, 215, 0.25),
  cycleHandle: "rgba(150,245,90,.85)",
  dropTarget: ok(0.82, 0.12, 215, 0.12),
  stepCell: glowLine(0.07),
  hover: glowLine(0.08),
};

const lightLine: Record<LineKey, string> = {
  barLine: water(0.22),
  beatLine: water(0.08),
  rulerBar: water(0.38),
  rulerTick: water(0.22),
  cycleEdge: "#5a9d45",
  editorBar: water(0.2),
  editorBeat: water(0.08),
  waveform: "rgba(16,50,74,.8)",
  waveformMid: "rgba(16,50,74,.33)",
  midiNote: "rgba(16,50,74,.8)",
  clipName: "#123a55",
  clipTitleBottom: water(0.16),
  clipHighlight: white(0.85),
  clipContact: water(0.4),
  clipSelected: "#0b5a8c",
  cellDivider: water(0.12),
  dividerDark: sky(0.82, 0.03),
  laneTop: white(0.8),
  laneBottom: water(0.13),
  rulerBottom: sky(0.78, 0.035),
  faderLine: water(0.6),
  faderLineHi: white(0.95),
  milled: water(0.5),
  milledHi: white(0.95),
  dragGhostEdge: water(0.55),
  noteSelected: "#063d63",
  noteHighlight: white(0.6),
  splitGuide: ok(0.5, 0.125, 240, 0.9),
  staff: sky(0.66, 0.04),
  noteHead: ok(0.36, 0.05, 230),
  border: sky(0.76, 0.04),
  borderStrong: sky(0.64, 0.05),
  hairline: water(0.1),
};

const darkLine: Record<LineKey, string> = {
  barLine: glowLine(0.1),
  beatLine: glowLine(0.035),
  rulerBar: glowLine(0.24),
  rulerTick: glowLine(0.11),
  cycleEdge: "rgba(150,245,90,.75)",
  editorBar: glowLine(0.11),
  editorBeat: glowLine(0.04),
  waveform: white(0.8),
  waveformMid: white(0.33),
  midiNote: white(0.82),
  clipName: white(0.94),
  clipTitleBottom: black(0.25),
  clipHighlight: white(0.4),
  clipContact: black(0.5),
  clipSelected: "#d8f6ff",
  cellDivider: glowLine(0.08),
  dividerDark: black(0.55),
  laneTop: glowLine(0.04),
  laneBottom: black(0.45),
  rulerBottom: black(0.6),
  faderLine: "rgba(2,14,28,.7)",
  faderLineHi: white(0.5),
  milled: "rgba(2,14,28,.6)",
  milledHi: white(0.4),
  dragGhostEdge: glowLine(0.6),
  noteSelected: "#ffffff",
  noteHighlight: white(0.5),
  splitGuide: ok(0.82, 0.12, 215, 0.9),
  staff: glowLine(0.4),
  noteHead: white(0.92),
  border: "rgba(2,12,26,.75)",
  borderStrong: "rgba(130,200,255,.3)",
  hairline: glowLine(0.07),
};

const LIGHTS: Record<Mode, Light> = {
  light: {
    hi: (a) => white(Math.min(1, 0.6 + a * 1.4)),
    lo: (a) => water(Number((a * 0.5).toFixed(3))),
    glow: 0.8,
  },
  dark: {
    hi: (a) => `rgba(200,238,255,${Math.min(1, a * 2.2).toFixed(3)})`,
    lo: (a) => `rgba(0,6,16,${a})`,
    glow: 1.15,
  },
};

/** A lens face: reflection, waist, body, and a rim light rising from below. */
const lens = (
  top: string,
  upper: string,
  waist: string,
  lower: string,
  rim: string,
) =>
  `radial-gradient(ellipse at 50% 112%, ${rim} 0%, transparent 62%), linear-gradient(180deg, ${top} 0%, ${upper} 46%, ${waist} 50%, ${lower} 100%)`;

/** A diagonal sheen across wide glass bars. */
const sheen = (a: number) =>
  `linear-gradient(115deg, transparent 18%, ${white(a)} 19%, ${white(a * 0.12)} 26%, transparent 27%, transparent 66%, ${white(a * 0.7)} 67%, transparent 77%)`;

export function aero(mode: Mode): ThemeSpec {
  const dark = mode === "dark";
  const c = dark ? darkColor : lightColor;
  const fill = dark ? darkFill : lightFill;
  const line = dark ? darkLine : lightLine;
  const light = LIGHTS[mode];
  const gradient = physicalGradients(c);
  const shadow = physicalShadows(c, light);
  const rim = dark ? "rgba(70,190,255,.55)" : "rgba(255,255,255,.7)";

  const lime = dark
    ? lens("#c9f58f", "#6cb62c", "#1f6410", "#57a81f", "rgba(190,255,110,.95)")
    : lens("#e1f8bb", "#8ac253", "#3a7f1d", "#75b533", "#c4ff70");
  const limeHover = dark
    ? lens("#dcffa9", "#80cc3a", "#2a7716", "#6cc02b", "#d8ff8a")
    : lens("#ecffc9", "#9bd25f", "#468f23", "#88c843", "#e1ffa8");
  const limePressed = dark
    ? lens("#2b5a17", "#1d4a0e", "#2c6a17", "#4f9a2a", "rgba(170,245,100,.6)")
    : lens("#40692c", "#2b5818", "#3c7b22", "#6aad42", "#b6f279");

  gradient.raised = dark
    ? lens(
        sea(0.56, 0.05),
        sea(0.36, 0.05),
        sea(0.2, 0.045),
        sea(0.29, 0.05),
        rim,
      )
    : lens("#ffffff", "#d7e6eb", "#9bafba", "#c6dce5", rim);
  gradient.raisedHover = dark
    ? lens(
        sea(0.66, 0.06),
        ok(0.45, 0.09, 232),
        ok(0.27, 0.09, 238),
        ok(0.4, 0.1, 232),
        "rgba(110,215,255,.8)",
      )
    : lens("#ffffff", "#bde3f3", "#76afc9", "#bceafa", "#edffff");
  gradient.pressed = `linear-gradient(180deg, ${c.pressedTop}, ${c.pressedBottom})`;
  gradient.lit = lime;
  gradient.sendKey = lime;
  gradient.segment = dark
    ? lens(
        ok(0.78, 0.09, 220),
        ok(0.52, 0.13, 236),
        ok(0.34, 0.13, 244),
        ok(0.48, 0.14, 236),
        "rgba(140,225,255,.8)",
      )
    : lens("#f2fbff", "#a9d8ee", "#6aa9cb", "#a5dcf5", "#f1ffff");
  gradient.titleBar = "none";
  gradient.panelHeader = dark
    ? `${sheen(0.1)}, linear-gradient(180deg, ${sea(0.4, 0.055)}, ${sea(0.29, 0.05)} 48%, ${sea(0.2, 0.045)} 50%, ${sea(0.27, 0.05)})`
    : `${sheen(0.5)}, linear-gradient(180deg, #e9f3f4, #b1c9d0 48%, #89abb7 50%, #c2dce2)`;
  gradient.dialog = dark
    ? `linear-gradient(135deg, ${sea(0.29, 0.045)}, ${sea(0.22, 0.04)})`
    : "linear-gradient(135deg, #f4fcff, #e2f0f7)";
  gradient.dialogHeader = dark
    ? `${sheen(0.14)}, linear-gradient(180deg, #3f6f95, #1d4468 48%, #0c2a48 51%, #1f527a)`
    : `${sheen(0.3)}, linear-gradient(180deg, #8fc3e6, #3f8cc4 48%, #1c6aa8 51%, #4aa3d8)`;
  gradient.plate = dark
    ? `linear-gradient(180deg, ${sea(0.3, 0.042)}, ${sea(0.235, 0.036)})`
    : "linear-gradient(180deg, #f8fdff, #dcebf3)";
  gradient.eqGrid = eqGrid(fill.eqGridLine, fill.eqZeroLine);

  const edge = dark ? "rgba(1,10,24,.9)" : "#486a7a";
  shadow.raised = dark
    ? `inset 0 1px 0 rgba(215,243,255,.5), inset 0 -2px 3px rgba(0,8,20,.55), inset 0 0 0 1px rgba(150,215,255,.1), 0 0 0 1px ${edge}, 0 2px 4px rgba(0,5,14,.6)`
    : `inset 0 1px 0 #ffffffed, inset 0 -2px 3px #32526066, inset 1px 0 1px #ffffffb0, inset -1px 0 1px #ffffff70, 0 0 0 1px ${edge}, 0 2px 3px #132b4266`;
  shadow.raisedSm = dark
    ? `inset 0 1px 0 rgba(215,243,255,.4), 0 0 0 1px ${edge}, 0 1px 2px rgba(0,5,14,.5)`
    : "inset 0 1px 0 #fff, inset 0 0 0 1px #a0bac8, 0 1px 1px #31546c1f";
  shadow.pressed = dark
    ? `inset 0 2px 5px rgba(0,4,12,.8), inset 0 0 0 1px ${edge}, 0 1px 0 rgba(150,215,255,.14)`
    : "inset 0 1px 3px #31546c40, inset 0 0 0 1px #699eb9, 0 1px 0 #fff";
  shadow.lit = dark
    ? "inset 0 1px 1px #f4ffd6, inset 0 -2px 4px rgba(10,50,8,.7), 0 0 0 1px #0d3306, 0 0 14px rgba(150,245,90,.5), 0 0 28px rgba(150,245,90,.2)"
    : "inset 0 1px 1px #fbffe6, inset 0 -2px 4px #1c521a99, 0 0 0 1px #315d25, 0 2px 4px #112c2460";
  shadow.sendKey = shadow.lit;
  shadow.segment = dark
    ? `inset 0 1px 0 rgba(225,246,255,.6), inset 0 0 0 1px rgba(8,30,70,.9), 0 0 10px rgba(80,180,255,.35)`
    : "inset 0 1px 0 #fff, inset 0 0 0 1px #5b93b5, 0 1px 2px #31546c33";
  shadow.groove = dark
    ? `inset 0 1px 3px rgba(0,4,12,.85), inset 0 0 0 1px rgba(0,6,16,.7), 0 1px 0 rgba(150,215,255,.12)`
    : "inset 0 1px 3px #31546c33, inset 0 0 0 1px #a6bfcc, 0 1px 0 #fff";
  shadow.wellInput = dark
    ? `inset 0 1px 3px rgba(0,4,12,.7), inset 0 0 0 1px rgba(0,6,16,.75), 0 1px 0 rgba(150,215,255,.1)`
    : "inset 0 1px 3px #31546c20, inset 0 0 0 1px #a6bfcc";
  shadow.wellDeep = dark
    ? `inset 0 2px 6px #000, inset 0 0 0 1px #00121f, 0 1px 0 rgba(150,215,255,.22), 0 0 0 1px rgba(1,10,24,.6)`
    : "inset 0 2px 5px #000b, 0 1px 0 #ffffffb0, 0 0 0 1px #0d2430";
  if (!dark) {
    shadow.panelAgent = "inset 1px 0 0 #a6c4d5, -3px 0 12px #30566b0a";
    shadow.panelLeft = "inset -1px 0 0 #b1cbd8";
    shadow.panelRight = "inset 1px 0 0 #b1cbd8";
    shadow.editorPane = "0 -2px 6px #30566b16, inset 0 1px 0 #fff";
    shadow.headerCell =
      "inset -1px 0 0 #b1cbd8, inset 0 -1px 0 #bed2dd, inset 0 1px 0 #fff";
    shadow.divider = "inset 0 -1px 0 #bfd4df";
    shadow.menu = "inset 0 1px 0 #fff, 0 0 0 1px #91b2c5, 0 8px 24px #23475d40";
    shadow.dialog =
      "inset 0 1px 0 #fff, 0 0 0 1px #6f9db8, 0 18px 55px #193a5266";
  } else {
    shadow.dialog = `inset 0 1px 0 rgba(200,238,255,.3), 0 0 0 1px rgba(1,10,24,.9), 0 0 0 2px rgba(120,200,255,.18), 0 24px 70px rgba(0,4,12,.75)`;
    shadow.menu = `inset 0 1px 0 rgba(200,238,255,.22), 0 0 0 1px rgba(1,10,24,.9), 0 12px 32px rgba(0,4,12,.7)`;
  }

  const vars: Record<string, string> = dark
    ? {
        "--aero-frame": `${sheen(0.1)}, radial-gradient(ellipse at 18% -20%, rgba(90,255,170,.4), transparent 58%), radial-gradient(ellipse at 88% 120%, rgba(60,150,255,.45), transparent 60%), linear-gradient(125deg, #041320, #0a2a4a 55%, #06303a)`,
        "--aero-frame-shadow":
          "inset 0 0 0 1px rgba(150,220,255,.28), inset 0 0 0 2px rgba(0,8,20,.7)",
        "--aero-title": `${sheen(0.12)}, linear-gradient(180deg, rgba(150,210,255,.22), rgba(20,60,100,.3) 50%, rgba(2,16,34,.35))`,
        "--aero-bar": `${sheen(0.1)}, linear-gradient(180deg, #3b5f80 0%, #17324e 46%, #07182c 50%, #12405c 100%)`,
        "--aero-bar-edge": "#010a16",
        "--aero-bar-shadow":
          "inset 0 1px 0 rgba(200,238,255,.45), inset 0 -1px 0 #000a14, 0 2px 8px rgba(0,4,12,.6)",
        "--aero-bar-ink": "#e6f6ff",
        "--aero-bar-ink-dim": "#a9cbe0",
        "--aero-bar-text-shadow": "0 1px 2px #000c18",
        "--aero-lcd": "linear-gradient(180deg, #01070f, #052033)",
        "--aero-lcd-glow": "0 0 8px rgba(90,215,255,.45)",
        "--aero-tabs": `linear-gradient(180deg, #2c4f6e, #10283f 48%, #061626 50%, #123752)`,
        "--aero-lime": lime,
        "--aero-lime-hover": limeHover,
        "--aero-lime-pressed": limePressed,
        "--aero-lime-ink": "#ffffff",
        "--aero-lime-text-shadow": "0 1px 2px #0c2406",
        "--aero-lime-shadow":
          "inset 0 1px 1px #f4ffd6, inset 0 -2px 3px rgba(10,50,8,.7), 0 0 0 1px #0a2a05, 0 0 14px rgba(150,245,90,.45), 0 3px 6px rgba(0,4,12,.6)",
        "--aero-gloss":
          "linear-gradient(180deg, rgba(255,255,255,.8), rgba(255,255,255,.1))",
        "--aero-composer": `linear-gradient(180deg, ${sea(0.27, 0.036)}, ${sea(0.22, 0.034)})`,
        "--aero-conversation": `linear-gradient(90deg, rgba(150,215,255,.05), transparent 24px), ${c.agentPanel}`,
        "--aero-input": sea(0.165, 0.03),
      }
    : {
        "--aero-frame": `${sheen(0.3)}, radial-gradient(ellipse at 20% -10%, #b9f08a, transparent 55%), radial-gradient(ellipse at 90% 115%, #9fe9ff, transparent 55%), linear-gradient(125deg, #1b69ad, #2c9bd3 46%, #3cb5ae 78%, #69c159)`,
        "--aero-frame-shadow":
          "inset 0 0 0 1px #e6fbffb0, inset 0 0 0 2px #0e3c5c80",
        // The title strip is tinted deep blue so white menu titles hold 4.5:1.
        "--aero-title": `${sheen(0.28)}, linear-gradient(180deg, rgba(255,255,255,.28), rgba(255,255,255,0) 50%), linear-gradient(180deg, rgba(5,48,92,.62), rgba(5,48,92,.4))`,
        "--aero-bar": `${sheen(0.3)}, linear-gradient(180deg, #7fa3bd 0%, #3b6384 46%, #1a3f60 50%, #3a7396 100%)`,
        "--aero-bar-edge": "#0f3450",
        "--aero-bar-shadow":
          "inset 0 1px 0 #f2fcffb0, inset 0 -1px 0 #08243a, 0 2px 5px #0e2c4455",
        "--aero-bar-ink": "#f4fbff",
        "--aero-bar-ink-dim": "#cfe4f1",
        "--aero-bar-text-shadow": "0 1px 2px rgba(3,30,58,.85)",
        "--aero-lcd": "linear-gradient(180deg, #04141c, #123226)",
        "--aero-lcd-glow": "0 0 6px rgba(150,235,90,.35)",
        "--aero-tabs":
          "linear-gradient(180deg, #86a9c2, #44708f 48%, #254f70 50%, #4b86a8)",
        "--aero-lime": lime,
        "--aero-lime-hover": limeHover,
        "--aero-lime-pressed": limePressed,
        "--aero-lime-ink": "#ffffff",
        "--aero-lime-text-shadow": "0 1px 2px #16310b",
        "--aero-lime-shadow":
          "inset 0 1px 1px #f4ffdc, inset 0 -2px 3px #1c521a99, 0 0 0 1px #254e17, 0 3px 5px #10241866",
        "--aero-gloss": "linear-gradient(180deg, #ffffffed, #ffffff26)",
        "--aero-composer": "linear-gradient(180deg, #eef7fb, #e0eef5)",
        "--aero-conversation": `linear-gradient(90deg, #fff8, transparent 24px), ${c.agentPanel}`,
        "--aero-input": "#fcfeff",
      };

  return {
    scheme: mode,
    color: c,
    gradient,
    shadow,
    fill,
    line,
    blur: { glass: "14px", glassAgent: "16px", glassCard: "18px" },
    radius: RADIUS,
    canvasShadow: physicalCanvasShadows(c, light),
    clipMix: dark
      ? { faceTop: 72, faceBottom: 48 }
      : { faceTop: 50, faceBottom: 74 },
    fontUi: FONT,
    capsTracking: "0.03em",
    vars: {
      ...vars,
      "--display-glow": dark ? "0.3" : "0.22",
      "--ring-glow": alpha(c.accent, dark ? 0.7 : 0.3),
    },
  };
}
