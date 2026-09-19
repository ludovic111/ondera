/**
 * The physical material recipes from design spec sheet 02, written once against
 * a light model so a theme can re-light them. Light falls from directly above:
 * specular highlight on the top edge, face darkening downward, contact line on
 * the bottom edge, then a soft drop shadow at 2–3× the contact distance.
 *
 * `hi(a)` is the highlight at the dark-theme strength `a`, `lo(a)` the shade.
 * On a light surface highlights saturate early and shade must be far weaker for
 * the same perceived depth, which is what the light models below encode.
 */
import type {
  CanvasShadowKey,
  CanvasShadowLayer,
  ColorKey,
  GradientKey,
  ShadowKey,
} from "./schema";
import { alpha } from "./schema";

export interface Light {
  hi: (a: number) => string;
  lo: (a: number) => string;
  /** Glow strength multiplier for accent and LED halos. */
  glow: number;
}

type C = Record<ColorKey, string>;

export const eqGrid = (grid: string, zero: string) =>
  `repeating-linear-gradient(90deg, ${grid} 0 1px, transparent 1px 42px), linear-gradient(180deg, transparent 36px, ${zero} 36px, ${zero} 37px, transparent 37px)`;

export function physicalGradients(c: C): Record<GradientKey, string> {
  const v = (a: string, b: string) => `linear-gradient(180deg, ${a}, ${b})`;
  return {
    raised: v(c.controlTop, c.controlBottom),
    raisedHover: v(c.segmentTop, c.segmentBottom),
    pressed: v(c.pressedTop, c.pressedBottom),
    lit: `radial-gradient(circle at 50% 30%, ${c.accentHi}, ${c.accentLo})`,
    transport: v(c.transportTop, c.transportBottom),
    titleBar: v(c.panel, c.panel),
    segment: v(c.segmentTop, c.segmentBottom),
    thumb: v(c.thumbTop, c.thumbBottom),
    knob: `radial-gradient(circle at 50% 28%, ${c.knobHi}, ${c.knobLo} 72%)`,
    knobBig: `radial-gradient(circle at 50% 25%, ${c.knobBigHi}, ${c.knobBigLo} 70%)`,
    knobInner: `radial-gradient(circle at 50% 35%, ${c.knobInnerHi}, ${c.knobInnerLo})`,
    faderCap: `linear-gradient(180deg, ${c.capTop}, ${c.capMid} 55%, ${c.capBottom})`,
    header: v(c.headerTop, c.headerBottom),
    headerSelected: v(c.headerSelectedTop, c.headerSelectedBottom),
    headerAgent: v(c.headerAgentTop, c.headerAgentBottom),
    panelHeader: v(c.panel, c.panel),
    insert: v(c.insertTop, c.insertBottom),
    chip: v(c.chipTop, c.chipBottom),
    sendKey: `radial-gradient(circle at 50% 30%, ${c.accentHi}, ${c.accentLo})`,
    note: v(c.noteTop, c.noteBottom),
    eqGrid: "none",
    dialog: v(c.transportTop, c.panel),
    dialogHeader: v(c.transportTop, c.transportBottom),
    plate: v(c.raised, c.panel),
  };
}

export function physicalShadows(c: C, l: Light): Record<ShadowKey, string> {
  const { hi, lo } = l;
  const ac = (a: number) => alpha(c.accent, Math.min(1, a * l.glow));
  const hot = (a: number) => alpha(c.ledHot, Math.min(1, a * l.glow));
  return {
    raised: `inset 0 1px 0 ${hi(0.09)}, inset 0 -1px 0 ${lo(0.35)}, 0 1px 2px ${lo(0.55)}, 0 3px 5px ${lo(0.25)}`,
    raisedSm: `inset 0 1px 0 ${hi(0.09)}, 0 1px 2px ${lo(0.55)}`,
    pressed: `inset 0 2px 4px ${lo(0.65)}, inset 0 1px 1px ${lo(0.5)}, 0 1px 0 ${hi(0.05)}`,
    lit: `inset 0 1px 0 ${hi(0.45)}, inset 0 -1px 0 ${lo(0.3)}, 0 1px 2px ${lo(0.6)}, 0 0 12px ${ac(0.55)}, 0 0 22px ${ac(0.25)}`,
    knob: `inset 0 1px 0 ${hi(0.16)}, inset 0 -1px 1px ${lo(0.6)}, 0 2px 3px ${lo(0.6)}, 0 4px 7px ${lo(0.3)}`,
    knobBig: `inset 0 1px 0 ${hi(0.18)}, inset 0 -2px 2px ${lo(0.6)}, 0 3px 4px ${lo(0.65)}, 0 8px 12px ${lo(0.35)}`,
    knobInner: `inset 0 1px 1px ${lo(0.7)}, 0 1px 0 ${hi(0.08)}`,
    knobIndicator: `inset 0 0 1px ${lo(0.9)}, 0 0 0 1px ${lo(0.35)}`,
    faderCap: `inset 0 1px 0 ${hi(0.25)}, inset 0 -1px 0 ${lo(0.5)}, 0 2px 3px ${lo(0.7)}, 0 5px 8px ${lo(0.35)}`,
    thumb: `inset 0 1px 0 ${hi(0.22)}, inset 0 -1px 0 ${lo(0.4)}, 0 1px 2px ${lo(0.6)}, 0 3px 4px ${lo(0.3)}`,
    groove: `inset 0 1px 3px ${lo(0.9)}, inset 0 0 0 1px ${lo(0.5)}, 0 1px 0 ${hi(0.05)}`,
    grooveSoft: `inset 0 1px 2px ${lo(0.85)}, 0 1px 0 ${hi(0.05)}`,
    grooveShallow: `inset 0 1px 3px ${lo(0.8)}, 0 1px 0 ${hi(0.05)}`,
    grooveSend: `inset 0 1px 2px ${lo(0.6)}, 0 1px 0 ${hi(0.04)}`,
    wellDeep: `inset 0 2px 5px ${lo(0.85)}, inset 0 0 0 1px ${lo(0.6)}, 0 1px 0 ${hi(0.06)}`,
    wellInput: `inset 0 2px 4px ${lo(0.8)}, inset 0 0 0 1px ${lo(0.6)}, 0 1px 0 ${hi(0.05)}`,
    wellValue: `inset 0 1px 3px ${lo(0.85)}`,
    segment: `inset 0 1px 0 ${hi(0.1)}, 0 1px 2px ${lo(0.5)}`,
    clip: `inset 0 1px 0 ${hi(0.22)}, inset 0 -1px 0 ${lo(0.4)}, 0 2px 3px ${lo(0.55)}, 0 5px 10px ${lo(0.3)}`,
    clipAgent: `0 0 0 1px ${c.accent}, 0 0 14px ${ac(0.45)}`,
    clipSelected: `0 0 0 1.5px ${c.ink100}`,
    ledLit: `0 0 4px ${c.ledGlow}, 0 0 9px ${c.ledGlowSoft}`,
    ledAccent: `0 0 5px ${hot(0.9)}, 0 0 10px ${hot(0.4)}`,
    ledOff: `inset 0 1px 0 ${lo(0.6)}`,
    ledEmpty: `inset 0 0 0 1px ${hi(0.12)}`,
    glass: `inset 0 1px 0 ${hi(0.15)}, 0 0 0 1px ${lo(0.4)}, 0 3px 8px ${lo(0.4)}`,
    glassAgent: `inset 0 1px 0 ${hi(0.18)}, 0 0 0 1px ${ac(0.45)}, 0 4px 12px ${lo(0.5)}, 0 0 18px ${ac(0.18)}`,
    glassCard: `inset 0 1px 0 ${hi(0.14)}, 0 0 0 1px ${ac(0.35)}, 0 6px 18px ${lo(0.45)}, 0 0 24px ${ac(0.12)}`,
    accentDot: `0 0 6px ${ac(0.9)}, 0 0 12px ${ac(0.4)}`,
    accentDotLg: `0 0 6px ${ac(0.9)}, 0 0 14px ${ac(0.45)}`,
    accentBar: `0 0 6px ${ac(0.7)}`,
    playhead: `0 0 5px ${ac(0.7)}, 0 0 14px ${ac(0.25)}`,
    playheadRuler: `0 0 6px ${ac(0.8)}`,
    headerCell: `inset -1px 0 0 ${lo(0.6)}, inset 0 -1px 0 ${lo(0.55)}, inset 0 1px 0 ${hi(0.04)}`,
    headerCellAgent: `inset 0 0 0 1px ${ac(0.55)}, inset 0 0 18px ${ac(0.12)}, inset -1px 0 0 ${lo(0.6)}, inset 0 -1px 0 ${lo(0.55)}, inset 0 1px 0 ${hi(0.04)}`,
    headerColumn: `inset -1px 0 0 ${lo(0.6)}`,
    colorStrip: `inset -1px 0 0 ${lo(0.4)}, inset 1px 0 0 ${hi(0.15)}`,
    swatch: `inset 0 1px 0 ${hi(0.25)}, 0 1px 1px ${lo(0.5)}`,
    swatchSm: `inset 0 1px 0 ${hi(0.2)}, 0 1px 1px ${lo(0.5)}`,
    trafficLight: `inset 0 1px 0 ${hi(0.12)}, 0 1px 1px ${lo(0.5)}`,
    titleBar: `inset 0 -1px 0 ${lo(0.5)}, inset 0 1px 0 ${hi(0.05)}`,
    transport: `inset 0 1px 0 ${hi(0.06)}, 0 2px 6px ${lo(0.45)}`,
    toolbar: `inset 0 -1px 0 ${lo(0.6)}, 0 1px 0 ${hi(0.03)}`,
    toolbarEditor: `inset 0 -1px 0 ${lo(0.6)}`,
    rulerCorner: `inset -1px 0 0 ${lo(0.6)}, inset 0 -1px 0 ${lo(0.6)}`,
    panelLeft: `inset -1px 0 0 ${lo(0.6)}, inset -2px 0 4px ${lo(0.25)}`,
    panelRight: `inset 1px 0 0 ${lo(0.6)}, inset 2px 0 4px ${lo(0.25)}`,
    panelAgent: `inset 1px 0 0 ${lo(0.65)}, inset 3px 0 6px ${lo(0.3)}`,
    panelAgentRail: `inset 1px 0 0 ${lo(0.65)}`,
    editorPane: `0 -3px 10px ${lo(0.5)}, inset 0 1px 0 ${hi(0.06)}`,
    keyColumn: `inset -1px 0 0 ${lo(0.7)}`,
    keyRow: `inset 0 -1px 0 ${lo(0.35)}`,
    divider: `inset 0 -1px 0 ${lo(0.5)}`,
    dividerTop: `inset 0 1px 0 ${hi(0.04)}`,
    logEntry: `inset 0 1px 0 ${hi(0.04)}, 0 1px 2px ${lo(0.35)}`,
    logChip: `inset 0 1px 0 ${hi(0.12)}, 0 1px 2px ${lo(0.5)}`,
    insert: `inset 0 1px 0 ${hi(0.08)}, 0 1px 2px ${lo(0.5)}`,
    insertEmpty: `inset 0 1px 2px ${lo(0.6)}, 0 1px 0 ${hi(0.04)}`,
    ledInsertOff: `inset 0 1px 1px ${lo(0.8)}`,
    window: `0 30px 80px ${lo(0.6)}, 0 0 0 1px ${hi(0.06)}`,
    sendKey: `inset 0 1px 0 ${hi(0.4)}, inset 0 -1px 0 ${lo(0.25)}, 0 1px 2px ${lo(0.6)}, 0 0 12px ${ac(0.4)}`,
    menu: `inset 0 1px 0 ${hi(0.08)}, 0 0 0 1px ${lo(0.6)}, 0 8px 24px ${lo(0.55)}, 0 2px 6px ${lo(0.4)}`,
    dialog: `inset 0 1px 0 ${hi(0.12)}, 0 0 0 1px ${lo(0.7)}, 0 18px 55px ${lo(0.6)}, 0 4px 12px ${lo(0.35)}`,
    inlineInput: `inset 0 1px 2px ${lo(0.7)}, 0 0 0 1px ${ac(0.6)}`,
    note: `inset 0 1px 0 ${hi(0.35)}, inset 0 -1px 0 ${lo(0.3)}, 0 1px 2px ${lo(0.6)}, 0 2px 4px ${lo(0.3)}`,
    faderLineHi: `0 1px 0 ${hi(0.14)}`,
    milledHi: `1px 0 0 ${hi(0.12)}`,
    plate: `inset 0 1px 0 ${hi(0.1)}, inset 0 -1px 0 ${lo(0.5)}, 0 0 0 1px ${lo(0.55)}, 0 2px 5px ${lo(0.4)}`,
    focus: `0 0 0 2px ${c.accent}`,
  };
}

export function physicalCanvasShadows(
  c: C,
  l: Light,
): Record<CanvasShadowKey, readonly CanvasShadowLayer[]> {
  const ac = (a: number) => alpha(c.accent, Math.min(1, a * l.glow));
  return {
    clipDrop: [
      { blur: 10, offsetY: 5, color: l.lo(0.3) },
      { blur: 3, offsetY: 2, color: l.lo(0.55) },
    ],
    playhead: [
      { blur: 14, offsetY: 0, color: ac(0.25) },
      { blur: 5, offsetY: 0, color: ac(0.7) },
    ],
    playheadRuler: [{ blur: 6, offsetY: 0, color: ac(0.8) }],
    playheadFlag: [{ blur: 4, offsetY: 0, color: ac(0.7) }],
    bubble: [{ blur: 8, offsetY: 3, color: l.lo(0.4) }],
    agentRing: [{ blur: 14, offsetY: 0, color: ac(0.45) }],
    agentNote: [
      { blur: 10, offsetY: 0, color: ac(0.4) },
      { blur: 5, offsetY: 0, color: ac(0.9) },
    ],
    noteSelected: [{ blur: 6, offsetY: 0, color: alpha(c.ink100, 0.5) }],
    note: [
      { blur: 4, offsetY: 2, color: l.lo(0.3) },
      { blur: 2, offsetY: 1, color: l.lo(0.6) },
    ],
  };
}

/** Flat paint: no glow, no drop. Used by themes that draw with lines only. */
export const NO_CANVAS_SHADOWS: Record<
  CanvasShadowKey,
  readonly CanvasShadowLayer[]
> = {
  clipDrop: [],
  playhead: [],
  playheadRuler: [],
  playheadFlag: [],
  bubble: [],
  agentRing: [],
  agentNote: [],
  noteSelected: [],
  note: [],
};
