import { beatsPerBar, beatsToBars, type Clip, type Session } from '@ondera/core';
import { canvasShadow, color, fill, line, radius, size, timeline, white } from '../theme/tokens';
import { withLightness } from '../theme/color';
import { cc, hline, monoFont, roundRectPath, withShadows } from './paint';

const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'] as const;

export const isBlackKey = (pitch: number) => NOTE_NAMES[pitch % 12]!.includes('#');

export function noteLabel(pitch: number): string {
  return `${NOTE_NAMES[pitch % 12]}${Math.floor(pitch / 12) - 1}`;
}

/** Lowest pitch shown: the octave C at or below the clip's lowest note. */
export function editorLowPitch(clip: Clip | null): number {
  if (!clip || clip.data.kind !== 'midi' || clip.data.notes.length === 0) return 36;
  let lo = 127;
  for (const n of clip.data.notes) if (n.pitch < lo) lo = n.pitch;
  return Math.max(0, Math.floor(lo / 12) * 12);
}

export function editorClip(state: Session): Clip | null {
  const id = state.view.editorClipId;
  return id ? (state.clips.find((c) => c.id === id) ?? null) : null;
}

export function drawPianoRoll(ctx: CanvasRenderingContext2D, w: number, h: number, state: Session): void {
  const clip = editorClip(state);
  const rulerH = size.editorRuler;
  const rowH = size.keyRow;
  const rows = size.keyRows;
  const ppb = timeline.editorPxPerBar;
  const bpb = beatsPerBar(state.transport.timeSignature);
  const pxPerBeat = ppb / bpb;
  const low = editorLowPitch(clip);
  const gridTop = rulerH;

  ctx.fillStyle = color.timelineEmpty;
  ctx.fillRect(0, 0, w, h);

  // Ruler strip.
  ctx.fillStyle = color.ruler;
  ctx.fillRect(0, 0, w, rulerH);
  hline(ctx, 0, rulerH - 1, w, line.rulerBottom);

  // Row shading: every other row lightened, black-key rows darkened.
  for (let r = 0; r < rows; r++) {
    const pitch = low + (rows - 1 - r);
    const y = gridTop + r * rowH;
    if (r % 2 === 0) {
      ctx.fillStyle = cc(fill.rowShade);
      ctx.fillRect(0, y, w, rowH);
    }
    if (isBlackKey(pitch)) {
      ctx.fillStyle = cc(fill.blackKeyRow);
      ctx.fillRect(0, y, w, rowH);
    }
  }

  // Bar and beat lines, bar numbers.
  const startBar = clip?.startBar ?? 0;
  const bars = Math.ceil(w / ppb) + 1;
  ctx.font = monoFont('small');
  ctx.textBaseline = 'top';
  ctx.textAlign = 'left';
  for (let b = 0; b < bars; b++) {
    const x = Math.round(b * ppb);
    ctx.fillStyle = cc(line.rulerBar);
    ctx.fillRect(x, 0, 1, rulerH);
    ctx.fillStyle = color.ink300;
    ctx.fillText(String(startBar + b + 1), x + 5, 3);
    ctx.fillStyle = cc(line.editorBar);
    ctx.fillRect(x, gridTop, 1, h - gridTop);
    ctx.fillStyle = cc(line.editorBeat);
    for (let k = 1; k < bpb; k++) ctx.fillRect(Math.round(x + k * pxPerBeat), gridTop, 1, h - gridTop);
  }

  // Notes.
  if (clip && clip.data.kind === 'midi') {
    const track = state.tracks.find((t) => t.id === clip.trackId);
    const hue = track?.color ?? color.noteTop;
    const top = withLightness(hue, 0.8);
    const bottom = withLightness(hue, 0.66);
    for (const n of clip.data.notes) {
      const r = rows - 1 - (n.pitch - low);
      if (r < 0 || r >= rows) continue;
      const x = Math.round(n.start * pxPerBeat) + 1;
      const nw = Math.max(3, Math.round(n.length * pxPerBeat) - 2);
      const y = gridTop + r * rowH + 0.5;
      const nh = rowH - 1;
      withShadows(ctx, canvasShadow.note, () => {
        ctx.fillStyle = bottom;
        roundRectPath(ctx, x, y, nw, nh, radius.xs);
        ctx.fill();
      });
      const grad = ctx.createLinearGradient(0, y, 0, y + nh);
      grad.addColorStop(0, top);
      grad.addColorStop(1, bottom);
      ctx.fillStyle = grad;
      roundRectPath(ctx, x, y, nw, nh, radius.xs);
      ctx.fill();
      ctx.save();
      roundRectPath(ctx, x, y, nw, nh, radius.xs);
      ctx.clip();
      hline(ctx, x, y, nw, white(0.35));
      hline(ctx, x, y + nh - 1, nw, color.desk);
      ctx.fillStyle = cc(fill.velocity);
      ctx.fillRect(x, y, Math.round((n.velocity / 127) * nw), nh);
      ctx.restore();
    }
  }

  // Playhead relative to the clip start.
  const posBars = beatsToBars(state.transport.positionBeats, state.transport.timeSignature);
  const px = Math.round((posBars - startBar) * ppb);
  if (px >= 0 && px <= w) {
    withShadows(ctx, canvasShadow.playheadRuler, () => {
      ctx.fillStyle = cc(color.accent);
      ctx.fillRect(px, 0, 1, h);
    });
  }
}
