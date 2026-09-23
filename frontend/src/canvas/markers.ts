import type { Marker, Session } from "@ondera/core";
import { color, fill, line, radius, size } from "../theme/tokens";
import { cc, roundRectPath, uiFont } from "./paint";
import {
  barToX,
  laneGeometry,
  markersWith,
  type LaneGeometry,
  type MarkerDrag,
} from "./timeline";

export type { MarkerDrag };

/** Label widths from the last paint, so a click can hit the whole flag. */
const labelWidths = new Map<string, number>();
const FALLBACK_LABEL = 48;
const PAD = 5;
const STRIPE = 3;

const flagWidth = (m: Marker) =>
  STRIPE + PAD * 2 + (labelWidths.get(m.id) ?? FALLBACK_LABEL);

/** The flag drawn at `x`, clipped by the next marker so neighbours never overlap. */
function flagSpan(list: Marker[], i: number, geo: LaneGeometry) {
  const m = list[i]!;
  const x = Math.round(barToX(m.bar, geo));
  const next = list[i + 1];
  const limit = next ? Math.round(barToX(next.bar, geo)) - 2 : Infinity;
  return { x, w: Math.max(STRIPE, Math.min(flagWidth(m), limit - x)) };
}

/** The marker whose flag is under (x, y) in ruler pixels. Later flags win ties. */
export function markerAt(
  state: Session,
  x: number,
  y: number,
  drag?: MarkerDrag,
): Marker | null {
  if (y < size.markerTop - 2 || y > size.markerTop + size.markerH + 2)
    return null;
  const geo = laneGeometry(state);
  const list = markersWith(state, drag);
  for (let i = list.length - 1; i >= 0; i--) {
    const { x: fx, w } = flagSpan(list, i, geo);
    if (x >= fx - 3 && x <= fx + w) return list[i]!;
  }
  return null;
}

/** Where the marker's name sits, for an inline rename field over the ruler. */
export function markerLabelRect(state: Session, marker: Marker) {
  const geo = laneGeometry(state);
  return {
    left: Math.round(barToX(marker.bar, geo)) + STRIPE,
    top: size.ruler - size.inlineInputH - 1,
    width: Math.max(90, (labelWidths.get(marker.id) ?? FALLBACK_LABEL) + 24),
  };
}

/** Flags in the ruler's lower half: a coloured stripe and the section name on a chip. */
export function drawMarkerFlags(
  ctx: CanvasRenderingContext2D,
  w: number,
  state: Session,
  drag?: MarkerDrag,
): void {
  const geo = laneGeometry(state);
  const list = markersWith(state, drag);
  ctx.font = uiFont("small", "semibold");
  ctx.textBaseline = "middle";
  ctx.textAlign = "left";
  const top = size.markerTop;
  const h = size.markerH;
  list.forEach((m, i) => {
    labelWidths.set(m.id, Math.ceil(ctx.measureText(m.name).width));
    const { x, w: fw } = flagSpan(list, i, geo);
    if (x + fw < 0 || x > w) return;
    ctx.fillStyle = cc(fill.markerFlag);
    roundRectPath(ctx, x, top, fw, h, radius.xs);
    ctx.fill();
    ctx.fillStyle = cc(m.color ?? line.marker);
    ctx.fillRect(x, top, STRIPE, h);
    ctx.fillRect(x, top + h, 1, size.ruler - top - h);
    if (fw > STRIPE + PAD) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(x + STRIPE, top, fw - STRIPE - 2, h);
      ctx.clip();
      ctx.fillStyle = cc(color.inkBright);
      ctx.fillText(m.name, x + STRIPE + PAD, top + h / 2 + 0.5);
      ctx.restore();
    }
  });
}
