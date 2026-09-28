import {
  beatsToBars,
  TEMPO_MAX,
  TEMPO_MIN,
  type Session,
  type TempoPoint,
} from "@ryolune/core";
import { canvasShadow, color, line, size } from "../theme/tokens";
import { withAlpha } from "../theme/color";
import { cc, hline, monoFont, withShadows } from "./paint";
import { barToX, drawGrid, laneGeometry } from "./timeline";

/**
 * The tempo track: the starting tempo from bar 1, then each change as a point where it takes
 * over, reached by a step or, for a ramp, by a straight line (tempo is linear in beats, and so
 * in pixels). A drag in progress is drawn instead of the committed point.
 */
export interface TempoDrag {
  /** The change being dragged, by its bar; 0 is the starting tempo. */
  from: number;
  bar: number;
  bpm: number;
}

/** What is under the pointer: a change (by bar), the starting tempo's line, or nothing. */
export type TempoHit = { bar: number } | null;

const INSET = 8;

/** The starting tempo as bar 0, then every change, with a drag applied, in bar order. */
export function tempoPoints(state: Session, drag?: TempoDrag): TempoPoint[] {
  const points: TempoPoint[] = [
    { bar: 0, bpm: state.transport.tempo },
    ...(state.tempoChanges ?? []),
  ].map((p) =>
    drag && Math.abs(p.bar - drag.from) < 1e-6
      ? { ...p, bar: drag.bar, bpm: drag.bpm }
      : p,
  );
  return points.sort((a, b) => a.bar - b.bar);
}

/** The BPM range the lane shows: every tempo in the song with some room, in tens. */
export function tempoRange(points: readonly TempoPoint[]): [number, number] {
  const bpms = points.map((p) => p.bpm);
  let lo = Math.floor((Math.min(...bpms) - 10) / 10) * 10;
  let hi = Math.ceil((Math.max(...bpms) + 10) / 10) * 10;
  lo = Math.max(TEMPO_MIN, lo);
  hi = Math.min(TEMPO_MAX, Math.max(hi, lo + 20));
  return [lo, hi];
}

export function yOfBpm(bpm: number, range: [number, number], h: number) {
  const [lo, hi] = range;
  return INSET + (1 - (bpm - lo) / (hi - lo)) * Math.max(1, h - INSET * 2);
}

export function bpmAtY(y: number, range: [number, number], h: number) {
  const [lo, hi] = range;
  const t = 1 - (y - INSET) / Math.max(1, h - INSET * 2);
  return lo + Math.min(1, Math.max(0, t)) * (hi - lo);
}

/** A tempo as the lane labels it: whole BPM, or one decimal when it has one. */
export const bpmLabel = (bpm: number) =>
  Number.isInteger(Math.round(bpm * 10) / 10)
    ? String(Math.round(bpm))
    : (Math.round(bpm * 10) / 10).toFixed(1);

/** The change under (x, y), nearest first; the starting tempo's line before the first. */
export function tempoAt(
  state: Session,
  x: number,
  y: number,
  h: number,
): TempoHit {
  const geo = laneGeometry(state);
  const points = tempoPoints(state);
  const range = tempoRange(points);
  let best: TempoHit = null;
  let bestDistance: number = size.tempoGrip;
  for (const p of points.slice(1)) {
    const d = Math.hypot(
      barToX(p.bar, geo) - x,
      yOfBpm(p.bpm, range, h) - y,
    );
    if (d <= bestDistance) {
      best = { bar: p.bar };
      bestDistance = d;
    }
  }
  if (best) return best;
  const first = points[1];
  const beforeFirst = !first || x < barToX(first.bar, geo) - size.tempoGrip;
  if (
    beforeFirst &&
    Math.abs(yOfBpm(points[0]!.bpm, range, h) - y) <= size.tempoGrip / 2 + 1
  )
    return { bar: 0 };
  return null;
}

export function drawTempoLane(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  drag?: TempoDrag,
): void {
  const geo = laneGeometry(state);
  const { transport } = state;
  ctx.fillStyle = color.timelineEmpty;
  ctx.fillRect(0, 0, w, h);
  drawGrid(ctx, w, h, geo, transport.timeSignature);
  hline(ctx, 0, h - 1, w, line.laneBottom);

  const points = tempoPoints(state, drag);
  const range = tempoRange(points);
  const x = (bar: number) => barToX(bar, geo);
  const y = (bpm: number) => yOfBpm(bpm, range, h);

  // The curve: steps and ramps, then the last tempo to the right edge.
  const path = new Path2D();
  path.moveTo(Math.min(x(0), 0), y(points[0]!.bpm));
  for (let i = 1; i < points.length; i++) {
    const p = points[i]!;
    if (!p.ramp) path.lineTo(x(p.bar), y(points[i - 1]!.bpm));
    path.lineTo(x(p.bar), y(p.bpm));
  }
  path.lineTo(w, y(points[points.length - 1]!.bpm));
  const area = new Path2D(path);
  area.lineTo(w, h);
  area.lineTo(Math.min(x(0), 0), h);
  area.closePath();
  ctx.fillStyle = cc(withAlpha(color.accent, 0.1));
  ctx.fill(area);
  ctx.strokeStyle = cc(color.accent);
  ctx.lineWidth = 1.5;
  ctx.stroke(path);

  // Points and their tempos; the starting tempo is labelled at the left edge.
  ctx.font = monoFont("small");
  ctx.textBaseline = "bottom";
  ctx.textAlign = "left";
  points.forEach((p, i) => {
    const px = x(p.bar);
    const py = y(p.bpm);
    const dragged = drag !== undefined && Math.abs(p.bar - drag.bar) < 1e-6;
    if (i > 0 && px >= -size.tempoGrip && px <= w + size.tempoGrip) {
      withShadows(ctx, dragged ? canvasShadow.playhead : [], () => {
        ctx.fillStyle = cc(color.accent);
        ctx.beginPath();
        ctx.arc(px, py, size.tempoPoint + (dragged ? 1 : 0), 0, Math.PI * 2);
        ctx.fill();
      });
    }
    const next = points[i + 1];
    const labelX = Math.max(4, px + 5);
    const room = next ? x(next.bar) - labelX : Infinity;
    if (room < 28 || labelX > w) return;
    const above = py - 4 > 12;
    ctx.textBaseline = above ? "bottom" : "top";
    ctx.fillStyle = dragged ? color.inkBright : color.ink300;
    ctx.fillText(bpmLabel(p.bpm), labelX, above ? py - 3 : py + 4);
  });

  // Playhead.
  const px = Math.round(
    x(beatsToBars(transport.positionBeats, transport.timeSignature)),
  );
  if (px >= 0 && px <= w) {
    ctx.fillStyle = cc(color.accent);
    ctx.fillRect(px, 0, 1, h);
  }
}
