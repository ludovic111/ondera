import { line } from "../theme/tokens";
import { cc } from "./paint";

/**
 * Draws a slice of a peak array as a symmetric bar waveform into the rect,
 * one column per CSS pixel. `first`..`last` are fractional indices into
 * `peaks`, so a trimmed or partially scrolled clip lines up with its audio.
 */
export function drawWaveform(
  ctx: CanvasRenderingContext2D,
  peaks: Float32Array,
  x: number,
  y: number,
  w: number,
  h: number,
  first: number,
  last: number,
): void {
  if (w <= 0 || h <= 0 || peaks.length === 0) return;
  const mid = y + h / 2;
  const half = h / 2;
  const perPx = (last - first) / w;

  ctx.fillStyle = cc(line.waveform);
  const cols = Math.floor(w);
  for (let px = 0; px < cols; px++) {
    const i0 = Math.floor(first + px * perPx);
    const i1 = Math.max(i0 + 1, Math.floor(first + (px + 1) * perPx));
    let peak = 0;
    for (let i = Math.max(0, i0); i < i1 && i < peaks.length; i++) {
      const v = peaks[i] ?? 0;
      if (v > peak) peak = v;
    }
    const a = Math.max(0.8, peak * half * 0.95);
    ctx.fillRect(x + px, mid - a, 1, a * 2);
  }
  ctx.fillStyle = cc(line.waveformMid);
  ctx.fillRect(x, Math.round(mid), cols, 1);
}
