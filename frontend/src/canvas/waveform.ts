import { line } from "../theme/tokens";
import { cc } from "./paint";

/** Max-pyramids per peak array: level n holds the maximum of 2^n peaks. */
const pyramids = new WeakMap<Float32Array, Float32Array[]>();
function pyramid(peaks: Float32Array): Float32Array[] {
  let levels = pyramids.get(peaks);
  if (levels) return levels;
  levels = [peaks];
  let prev = peaks;
  while (prev.length > 64) {
    const next = new Float32Array(Math.ceil(prev.length / 2));
    for (let i = 0; i < next.length; i++)
      next[i] = Math.max(prev[i * 2] ?? 0, prev[i * 2 + 1] ?? 0);
    levels.push(next);
    prev = next;
  }
  pyramids.set(peaks, levels);
  return levels;
}

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
  /** Gain per column (0 = first column), e.g. a clip's gain and fades. */
  gainAt?: (px: number) => number,
): void {
  if (w <= 0 || h <= 0 || peaks.length === 0) return;
  const mid = y + h / 2;
  const half = h / 2;
  const perPx = (last - first) / w;

  // Zoomed out, a column spans many peaks: read them from the level that leaves
  // at most two entries per column instead of scanning the raw array every frame.
  const levels = pyramid(peaks);
  const level = Math.min(
    levels.length - 1,
    Math.max(0, Math.floor(Math.log2(Math.max(1, perPx)))),
  );
  const data = levels[level]!;
  const scale = 1 / 2 ** level;

  ctx.fillStyle = cc(line.waveform);
  const cols = Math.floor(w);
  ctx.beginPath();
  for (let px = 0; px < cols; px++) {
    const i0 = Math.floor((first + px * perPx) * scale);
    const i1 = Math.max(i0 + 1, Math.ceil((first + (px + 1) * perPx) * scale));
    let peak = 0;
    for (let i = Math.max(0, i0); i < i1 && i < data.length; i++) {
      const v = data[i]!;
      if (v > peak) peak = v;
    }
    if (gainAt) peak *= gainAt(px);
    const a = Math.max(0.8, Math.min(half, peak * half * 0.95));
    ctx.rect(x + px, mid - a, 1, a * 2);
  }
  ctx.fill();
  ctx.fillStyle = cc(line.waveformMid);
  ctx.fillRect(x, Math.round(mid), cols, 1);
}
