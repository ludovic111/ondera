import { line } from '../theme/tokens';
import { cc } from './paint';

/**
 * Draws a peak array as a symmetric bar waveform into the rect, one column
 * per CSS pixel. `startFrac`/`endFrac` select the visible slice of the peaks
 * so a partially scrolled clip still lines up.
 */
export function drawWaveform(
  ctx: CanvasRenderingContext2D,
  peaks: Float32Array,
  x: number,
  y: number,
  w: number,
  h: number,
  startFrac = 0,
  endFrac = 1,
): void {
  if (w <= 0 || h <= 0 || peaks.length === 0) return;
  const mid = y + h / 2;
  const half = h / 2;
  const first = startFrac * peaks.length;
  const span = (endFrac - startFrac) * peaks.length;
  const perPx = span / w;

  ctx.fillStyle = cc(line.waveform);
  const cols = Math.floor(w);
  for (let px = 0; px < cols; px++) {
    const i0 = Math.floor(first + px * perPx);
    const i1 = Math.max(i0 + 1, Math.floor(first + (px + 1) * perPx));
    let peak = 0;
    for (let i = i0; i < i1 && i < peaks.length; i++) {
      const v = peaks[i] ?? 0;
      if (v > peak) peak = v;
    }
    const a = Math.max(0.8, peak * half);
    ctx.fillRect(x + px, mid - a, 1, a * 2);
  }
  ctx.fillStyle = cc(line.waveformMid);
  ctx.fillRect(x, Math.round(mid), cols, 1);
}
