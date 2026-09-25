import {
  beatsToBars,
  type Clip,
  type Controller,
  type ControllerKind,
  type Session,
} from "@ondera/core";
import { color, fill, line, size } from "../theme/tokens";
import { withLightness } from "../theme/color";
import { cc, hline, monoFont } from "./paint";
import { rollGeometry, type RollGeometry } from "./pianoRoll";

/** One controller lane: a kind, a number for control changes, and a MIDI channel (absent: 0). */
export interface Lane {
  kind: ControllerKind;
  number?: number;
  channel?: number;
}

/** The lanes the selector offers first, most useful first. */
export const LANE_CHOICES: readonly Lane[] = [
  { kind: "cc", number: 1 },
  { kind: "cc", number: 11 },
  { kind: "cc", number: 64 },
  { kind: "bend" },
  { kind: "pressure" },
  { kind: "cc", number: 7 },
  { kind: "cc", number: 10 },
  { kind: "cc", number: 2 },
];

const CC_NAMES: Record<number, string> = {
  1: "Mod Wheel",
  2: "Breath",
  7: "Volume",
  10: "Pan",
  11: "Expression",
  64: "Sustain",
};

export function laneTitle(lane: Lane): string {
  const suffix = lane.channel ? ` · Ch ${lane.channel + 1}` : "";
  if (lane.kind === "bend") return `Pitch Bend${suffix}`;
  if (lane.kind === "pressure") return `Pressure${suffix}`;
  const name = CC_NAMES[lane.number ?? -1];
  return (name ? `${name} (CC${lane.number})` : `CC${lane.number}`) + suffix;
}

export const sameLane = (a: Lane, b: Lane) =>
  a.kind === b.kind &&
  (a.kind !== "cc" || a.number === b.number) &&
  (a.channel ?? 0) === (b.channel ?? 0);

export const laneOf = (p: Controller): Lane => {
  const lane: Lane =
    p.kind === "cc" ? { kind: "cc", number: p.number ?? 0 } : { kind: p.kind };
  if (p.channel) lane.channel = p.channel;
  return lane;
};

/** Parameters naming a lane for the controller.* commands. */
export const laneParams = (lane: Lane): Record<string, unknown> => {
  const params: Record<string, unknown> =
    lane.kind === "cc"
      ? { kind: "cc", number: lane.number }
      : { kind: lane.kind };
  if (lane.channel) params.channel = lane.channel;
  return params;
};

export function laneRange(lane: Lane): [number, number] {
  return lane.kind === "bend" ? [-8192, 8191] : [0, 127];
}

/** The base line sits at value 0: the centre of a bend lane, the bottom of the others. */
const BASE_VALUE = 0;

/** A number typed as "CC 74", "74" or "cc74", when it names a usable controller. */
export function parseCcNumber(text: string): number | null {
  const match = /^\s*(?:cc\s*)?(\d{1,3})\s*$/i.exec(text);
  if (!match) return null;
  const n = Number(match[1]);
  return n >= 0 && n <= 119 ? n : null;
}

/** Points of one lane in a clip, in time order. */
export function lanePoints(clip: Clip | null, lane: Lane): Controller[] {
  if (!clip || clip.data.kind !== "midi") return [];
  return (clip.data.controllers ?? [])
    .filter((p) => sameLane(laneOf(p), lane))
    .sort((a, b) => a.time - b.time);
}

/** Lanes that hold points in a clip, in the order they first appear. */
export function lanesInClip(clip: Clip | null): Lane[] {
  const out: Lane[] = [];
  const add = (lane: Lane) => {
    if (!out.some((l) => sameLane(l, lane))) out.push(lane);
  };
  if (clip?.data.kind === "midi")
    for (const p of clip.data.controllers ?? []) add(laneOf(p));
  return out;
}

/** Vertical layout: a small inset keeps the extreme values grabbable. */
const INSET = size.controllerInset;
export function yOfValue(lane: Lane, value: number, height: number): number {
  const [low, high] = laneRange(lane);
  const t = (value - low) / (high - low);
  return INSET + (1 - t) * Math.max(1, height - INSET * 2);
}
export function valueAtY(lane: Lane, y: number, height: number): number {
  const [low, high] = laneRange(lane);
  const t = 1 - (y - INSET) / Math.max(1, height - INSET * 2);
  const value = Math.round(low + Math.min(1, Math.max(0, t)) * (high - low));
  // The bend wheel snaps to its centre near the middle, as a real one does.
  if (lane.kind === "bend" && Math.abs(value) < 128) return 0;
  return value;
}

/** The point under (x, y), nearest first, within the grip distance. */
export function hitPoint(
  points: readonly Controller[],
  lane: Lane,
  pxPerBeat: number,
  height: number,
  x: number,
  y: number,
): Controller | null {
  let best: Controller | null = null;
  let bestDistance: number = size.controllerGrip;
  for (const p of points) {
    const d = Math.hypot(
      p.time * pxPerBeat - x,
      yOfValue(lane, p.value, height) - y,
    );
    if (d <= bestDistance) {
      best = p;
      bestDistance = d;
    }
  }
  return best;
}

export interface Sample {
  beat: number;
  value: number;
}

/**
 * A drawn stroke as lane points: one per grid step the stroke crossed, holding
 * the last value drawn in that step, and the range they replace. Nothing when
 * the stroke never entered the clip.
 */
export function strokePoints(
  samples: readonly Sample[],
  step: number,
  lengthBeats: number,
): { points: Sample[]; from: number; to: number } | null {
  const cells = new Map<number, number>();
  // Fill between samples so a fast stroke leaves no holes.
  for (let i = 0; i < samples.length; i++) {
    const a = samples[i]!;
    const b = samples[i + 1] ?? a;
    const from = Math.min(a.beat, b.beat);
    const to = Math.max(a.beat, b.beat);
    for (let t = Math.floor(from / step) * step; t <= to; t += step) {
      const cell = Math.round(t / step);
      if (cell * step < 0 || cell * step >= lengthBeats) continue;
      const k = b.beat === a.beat ? 0 : (t - a.beat) / (b.beat - a.beat);
      const clamped = Math.min(1, Math.max(0, k));
      cells.set(cell, Math.round(a.value + (b.value - a.value) * clamped));
    }
  }
  if (cells.size === 0) return null;
  const keys = [...cells.keys()].sort((x, y) => x - y);
  const points = keys.map((cell) => ({
    beat: cell * step,
    value: cells.get(cell)!,
  }));
  const from = keys[0]! * step;
  const to = Math.min(lengthBeats, (keys[keys.length - 1]! + 1) * step);
  return { points, from, to };
}

/** Plain text for a value: bend in semitones of the usual two-semitone range. */
export function valueLabel(lane: Lane, value: number): string {
  if (lane.kind === "bend") {
    const semitones = (value / 8192) * 2;
    return `${semitones >= 0 ? "+" : ""}${semitones.toFixed(2)} st`;
  }
  if (lane.kind === "cc" && lane.number === 64)
    return value >= 64 ? "down" : "up";
  return String(value);
}

export interface LaneOverlay {
  /** A point being dragged: its id and where it would land. */
  drag?: { id: string; time: number; value: number };
  /** A stroke being drawn, as it would be committed. */
  stroke?: Sample[];
  hover?: string;
}

/** Paint one lane with the geometry of the piano roll above it. */
export function drawControllerLane(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  lane: Lane,
  overlay: LaneOverlay = {},
): RollGeometry {
  const geo = rollGeometry(state, w);
  const { clip, ppb, pxPerBeat, bpb } = geo;
  ctx.fillStyle = color.timelineEmpty;
  ctx.fillRect(0, 0, w, h);
  hline(ctx, 0, 0, w, line.rulerBottom);

  const bars = Math.ceil(w / ppb) + 1;
  for (let b = 0; b < bars; b++) {
    const x = Math.round(b * ppb);
    ctx.fillStyle = cc(line.editorBar);
    ctx.fillRect(x, 1, 1, h - 1);
    ctx.fillStyle = cc(line.editorBeat);
    for (let k = 1; k < bpb; k++)
      ctx.fillRect(Math.round(x + k * pxPerBeat), 1, 1, h - 1);
  }
  const lengthBeats = clip ? clip.lengthBars * bpb : 0;
  if (clip) {
    const endX = Math.round(lengthBeats * pxPerBeat);
    if (endX < w) {
      ctx.fillStyle = cc(fill.blackKeyRow);
      ctx.fillRect(endX, 1, w - endX, h - 1);
    }
  }
  const baseY = Math.round(yOfValue(lane, BASE_VALUE, h));
  hline(ctx, 0, baseY, w, line.editorBar);

  const track = clip
    ? state.tracks.find((t) => t.id === clip.trackId)
    : undefined;
  const hue = track?.color ?? color.noteTop;
  const top = withLightness(hue, 0.8);
  const bottom = withLightness(hue, 0.66);

  let points = lanePoints(clip, lane);
  if (overlay.drag) {
    const d = overlay.drag;
    points = points
      .map((p) => (p.id === d.id ? { ...p, time: d.time, value: d.value } : p))
      .sort((a, b) => a.time - b.time);
  }
  if (overlay.stroke && overlay.stroke.length > 0) {
    const from = overlay.stroke[0]!.beat;
    const to = overlay.stroke[overlay.stroke.length - 1]!.beat;
    points = points.filter((p) => p.time < from || p.time > to);
  }

  // Held values: a filled step from each point to the next, or to the clip end.
  for (let i = 0; i < points.length; i++) {
    const p = points[i]!;
    const next = points[i + 1]?.time ?? lengthBeats;
    const x0 = Math.round(p.time * pxPerBeat);
    const x1 = Math.round(Math.min(next, lengthBeats) * pxPerBeat);
    const y = Math.round(yOfValue(lane, p.value, h));
    ctx.fillStyle = cc(fill.velocity);
    ctx.fillRect(x0, Math.min(y, baseY), x1 - x0, Math.abs(baseY - y));
    ctx.fillStyle = top;
    ctx.fillRect(x0, y, Math.max(1, x1 - x0), 1);
  }
  // Stems and heads.
  for (const p of points) {
    const x = Math.round(p.time * pxPerBeat);
    const y = Math.round(yOfValue(lane, p.value, h));
    const active = overlay.drag?.id === p.id || overlay.hover === p.id;
    const head = active ? cc(color.accent) : p.agent ? cc(color.accentHi) : top;
    ctx.fillStyle = active ? cc(color.accent) : bottom;
    ctx.fillRect(x, Math.min(y, baseY), 1, Math.abs(baseY - y) + 1);
    ctx.fillStyle = head;
    ctx.beginPath();
    ctx.arc(x + 0.5, y + 0.5, size.controllerPoint, 0, Math.PI * 2);
    ctx.fill();
  }
  // A stroke in progress.
  if (overlay.stroke) {
    for (const s of overlay.stroke) {
      const x = Math.round(s.beat * pxPerBeat);
      const y = Math.round(yOfValue(lane, s.value, h));
      ctx.fillStyle = cc(fill.pencilPreview);
      ctx.fillRect(x, Math.min(y, baseY), 1, Math.abs(baseY - y) + 1);
      ctx.fillStyle = cc(color.accent);
      ctx.fillRect(x - 1, y - 1, 3, 3);
    }
  }

  // The lane's name and the value under the pointer.
  ctx.font = monoFont("small");
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  ctx.fillStyle = color.ink500;
  const shown =
    overlay.drag ??
    (overlay.hover ? points.find((p) => p.id === overlay.hover) : undefined);
  ctx.fillText(
    shown
      ? `${laneTitle(lane)} · ${valueLabel(lane, shown.value)}`
      : laneTitle(lane),
    5,
    4,
  );

  // Playhead relative to the clip start.
  if (clip) {
    const posBars = beatsToBars(
      state.transport.positionBeats,
      state.transport.timeSignature,
    );
    const px = Math.round((posBars - clip.startBar) * ppb);
    if (px >= 0 && px <= w) {
      ctx.fillStyle = cc(color.accent);
      ctx.fillRect(px, 0, 1, h);
    }
  }
  return geo;
}
