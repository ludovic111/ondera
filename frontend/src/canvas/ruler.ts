import {
  beatLineOffsets,
  beatsToBarBeat,
  beatsToBars,
  formatBarBeatShort,
  type Session,
} from "@ondera/core";
import {
  canvasShadow,
  color,
  fill,
  line,
  radius,
  size,
  timeline,
} from "../theme/tokens";
import { cc, hline, monoFont, roundRectPath, withShadows } from "./paint";
import { barToX, laneGeometry } from "./timeline";

/** Cycle range being dragged; drawn instead of the committed one. */
export interface RulerOverlay {
  cycle?: { startBar: number; endBar: number };
}

export function drawRuler(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  overlay: RulerOverlay = {},
): void {
  const geo = laneGeometry(state);
  const { transport } = state;

  ctx.fillStyle = color.ruler;
  ctx.fillRect(0, 0, w, h);

  const cyc =
    overlay.cycle ??
    (transport.cycle
      ? { startBar: transport.cycleStartBar, endBar: transport.cycleEndBar }
      : null);
  if (cyc) {
    const x0 = Math.round(barToX(cyc.startBar, geo));
    const x1 = Math.round(barToX(cyc.endBar, geo));
    ctx.fillStyle = cc(fill.cycleRuler);
    ctx.fillRect(x0, 0, x1 - x0, h);
    ctx.fillStyle = cc(line.cycleEdge);
    ctx.fillRect(x0, 0, 1, h);
    ctx.fillRect(x1 - 1, 0, 1, h);
    // Grip handles.
    ctx.fillStyle = cc(fill.cycleHandle);
    ctx.fillRect(x0, 0, size.cycleGrip, 3);
    ctx.fillRect(x1 - size.cycleGrip, 0, size.cycleGrip, 3);
  }

  const labelEvery =
    geo.ppb >= 40 ? 1 : geo.ppb >= 20 ? 2 : geo.ppb >= 10 ? 4 : 8;
  const beats = beatLineOffsets(geo.ppb, transport.timeSignature);
  const firstBar = Math.floor(geo.scrollBars);
  const lastBar = Math.ceil(geo.scrollBars + w / geo.ppb);
  ctx.font = monoFont("value");
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  for (let bar = firstBar; bar <= lastBar; bar++) {
    const x = Math.round(barToX(bar, geo));
    ctx.fillStyle = cc(line.rulerBar);
    ctx.fillRect(x, 0, 1, h);
    if (bar % labelEvery === 0) {
      ctx.fillStyle = color.ink300;
      ctx.fillText(String(bar + 1), x + 5, 4);
    }
    ctx.fillStyle = cc(line.rulerTick);
    for (const offset of beats) {
      ctx.fillRect(
        Math.round(x + offset),
        h - timeline.rulerTickH,
        1,
        timeline.rulerTickH,
      );
    }
  }

  hline(ctx, 0, h - 1, w, line.rulerBottom);

  // Playhead: line, flag, and position bubble.
  const posBars = beatsToBars(transport.positionBeats, transport.timeSignature);
  const px = Math.round(barToX(posBars, geo));
  if (px < -timeline.playheadFlagW || px > w + timeline.playheadFlagW) return;

  withShadows(ctx, canvasShadow.playheadRuler, () => {
    ctx.fillStyle = cc(color.accent);
    ctx.fillRect(px, 0, 1, h);
  });
  withShadows(ctx, canvasShadow.playheadFlag, () => {
    ctx.fillStyle = cc(color.accent);
    ctx.beginPath();
    ctx.moveTo(px - timeline.playheadFlagW / 2, 0);
    ctx.lineTo(px + timeline.playheadFlagW / 2 + 1, 0);
    ctx.lineTo(px + 0.5, timeline.playheadFlagH);
    ctx.closePath();
    ctx.fill();
  });

  const label = formatBarBeatShort(
    beatsToBarBeat(transport.positionBeats, transport.timeSignature),
  );
  ctx.font = monoFont("value");
  const tw = ctx.measureText(label).width;
  const bx = px + 9;
  const by = 4;
  const bw = Math.ceil(tw) + 14;
  const bh = 18;
  withShadows(ctx, canvasShadow.bubble, () => {
    ctx.fillStyle = cc(fill.glass);
    roundRectPath(ctx, bx, by, bw, bh, radius.md);
    ctx.fill();
  });
  ctx.strokeStyle = cc(line.border);
  ctx.lineWidth = 1;
  roundRectPath(ctx, bx + 0.5, by + 0.5, bw - 1, bh - 1, radius.md);
  ctx.stroke();
  ctx.fillStyle = cc(line.hairline);
  ctx.fillRect(bx + 2, by + 1, bw - 4, 1);
  ctx.fillStyle = color.inkBright;
  ctx.textBaseline = "middle";
  ctx.fillText(label, bx + 7, by + bh / 2 + 0.5);
}
