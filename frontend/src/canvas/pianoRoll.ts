import {
  beatsPerBar,
  beatsToBars,
  type Clip,
  type Note,
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
import { withLightness } from "../theme/color";
import {
  cc,
  hline,
  monoFont,
  roundRectPath,
  uiFont,
  withShadows,
} from "./paint";

const NOTE_NAMES = [
  "C",
  "C#",
  "D",
  "D#",
  "E",
  "F",
  "F#",
  "G",
  "G#",
  "A",
  "A#",
  "B",
] as const;

export const isBlackKey = (pitch: number) =>
  NOTE_NAMES[pitch % 12]!.includes("#");

export function noteLabel(pitch: number): string {
  return `${NOTE_NAMES[pitch % 12]}${Math.floor(pitch / 12) - 1}`;
}

/** Lowest pitch shown: the octave C at or below the clip's lowest note. */
export function editorLowPitch(clip: Clip | null): number {
  if (!clip || clip.data.kind !== "midi" || clip.data.notes.length === 0)
    return 48;
  let lo = 127;
  for (const n of clip.data.notes) if (n.pitch < lo) lo = n.pitch;
  return Math.max(
    0,
    Math.min(127 - size.keyRows + 1, Math.floor(lo / 12) * 12),
  );
}

export function editorClip(state: Session): Clip | null {
  const id = state.view.editorClipId;
  return id ? (state.clips.find((c) => c.id === id) ?? null) : null;
}

export interface RollGeometry {
  clip: Clip | null;
  /** Pixels per bar in the editor. */
  ppb: number;
  pxPerBeat: number;
  rowH: number;
  rows: number;
  low: number;
  gridTop: number;
  bpb: number;
}

export function rollGeometry(state: Session, width: number): RollGeometry {
  const clip = editorClip(state);
  const bpb = beatsPerBar(state.transport.timeSignature);
  const bars = Math.max(1, clip?.lengthBars ?? 4);
  const ppb = Math.max(1, width / bars);
  return {
    clip,
    ppb,
    pxPerBeat: ppb / bpb,
    rowH: size.keyRow,
    rows: size.keyRows,
    low: state.view.editorLowPitch ?? editorLowPitch(clip),
    gridTop: size.editorRuler,
    bpb,
  };
}

export const pitchAtY = (geo: RollGeometry, y: number) =>
  geo.low + (geo.rows - 1 - Math.floor((y - geo.gridTop) / geo.rowH));
export const beatAtX = (geo: RollGeometry, x: number) => x / geo.pxPerBeat;
export const yOfPitch = (geo: RollGeometry, pitch: number) =>
  geo.gridTop + (geo.rows - 1 - (pitch - geo.low)) * geo.rowH;

/** Note under (x, y) and whether the cursor is on its right edge. */
export function hitTestNote(
  geo: RollGeometry,
  x: number,
  y: number,
): { note: Note; edge: boolean } | null {
  if (!geo.clip || geo.clip.data.kind !== "midi" || y < geo.gridTop)
    return null;
  const pitch = pitchAtY(geo, y);
  const beat = beatAtX(geo, x);
  const notes = geo.clip.data.notes;
  for (let i = notes.length - 1; i >= 0; i--) {
    const n = notes[i]!;
    if (n.pitch !== pitch || beat < n.start || beat >= n.start + n.length)
      continue;
    const endX = (n.start + n.length) * geo.pxPerBeat;
    return {
      note: n,
      edge:
        endX - x <= size.noteEdgeGrip &&
        n.length * geo.pxPerBeat > size.noteEdgeGrip * 2,
    };
  }
  return null;
}

export interface RollOverlay {
  ghost?: { start: number; length: number; pitch: number };
  pencil?: { start: number; length: number; pitch: number };
}

export function drawPianoRoll(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  overlay: RollOverlay = {},
): void {
  const geo = rollGeometry(state, w);
  const { clip, ppb, pxPerBeat, rowH, rows, low, gridTop, bpb } = geo;
  const mode = state.view.editorMode;

  ctx.fillStyle = color.timelineEmpty;
  ctx.fillRect(0, 0, w, h);

  // Ruler strip.
  ctx.fillStyle = color.ruler;
  ctx.fillRect(0, 0, w, gridTop);
  hline(ctx, 0, gridTop - 1, w, line.rulerBottom);

  if (mode === "score") {
    drawScore(ctx, w, h, state, geo);
    return;
  }

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
  ctx.font = monoFont("small");
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  const stepBeats = 4 / state.transport.snapDivision;
  for (let b = 0; b < bars; b++) {
    const x = Math.round(b * ppb);
    ctx.fillStyle = cc(line.rulerBar);
    ctx.fillRect(x, 0, 1, gridTop);
    ctx.fillStyle = color.ink300;
    ctx.fillText(String(startBar + b + 1), x + 5, 3);
    ctx.fillStyle = cc(line.editorBar);
    ctx.fillRect(x, gridTop, 1, h - gridTop);
    ctx.fillStyle = cc(line.editorBeat);
    for (let k = 1; k < bpb; k++)
      ctx.fillRect(Math.round(x + k * pxPerBeat), gridTop, 1, h - gridTop);
    if (mode === "step" && stepBeats * pxPerBeat >= 6) {
      ctx.fillStyle = cc(fill.stepCell);
      for (let k = stepBeats; k < bpb; k += stepBeats)
        ctx.fillRect(Math.round(x + k * pxPerBeat), gridTop, 1, h - gridTop);
    }
  }

  // Clip end shade.
  if (clip) {
    const endX = Math.round(clip.lengthBars * ppb);
    if (endX < w) {
      ctx.fillStyle = cc(fill.blackKeyRow);
      ctx.fillRect(endX, gridTop, w - endX, h - gridTop);
    }
  }

  // Notes.
  if (clip && clip.data.kind === "midi") {
    const track = state.tracks.find((t) => t.id === clip.trackId);
    const hue = track?.color ?? color.noteTop;
    const top = withLightness(hue, 0.8);
    const bottom = withLightness(hue, 0.66);
    for (const n of clip.data.notes) {
      const r = rows - 1 - (n.pitch - low);
      if (r < 0 || r >= rows) continue;
      const selected = state.view.selectedNoteId === n.id;
      const x = Math.round(n.start * pxPerBeat) + 1;
      const nw = Math.max(
        3,
        Math.round(
          (mode === "step" ? Math.max(n.length, stepBeats) : n.length) *
            pxPerBeat,
        ) - 2,
      );
      const y = gridTop + r * rowH + 0.5;
      const nh = rowH - 1;
      withShadows(
        ctx,
        selected ? canvasShadow.noteSelected : canvasShadow.note,
        () => {
          ctx.fillStyle = bottom;
          roundRectPath(ctx, x, y, nw, nh, radius.xs);
          ctx.fill();
        },
      );
      const grad = ctx.createLinearGradient(0, y, 0, y + nh);
      grad.addColorStop(0, n.agent ? cc(color.accentHi) : top);
      grad.addColorStop(1, n.agent ? cc(color.accentLo) : bottom);
      ctx.fillStyle = grad;
      roundRectPath(ctx, x, y, nw, nh, radius.xs);
      ctx.fill();
      ctx.save();
      roundRectPath(ctx, x, y, nw, nh, radius.xs);
      ctx.clip();
      hline(ctx, x, y, nw, line.noteHighlight);
      hline(ctx, x, y + nh - 1, nw, color.desk);
      ctx.fillStyle = cc(fill.velocity);
      ctx.fillRect(x, y, Math.round((n.velocity / 127) * nw), nh);
      ctx.restore();
      if (selected) {
        ctx.strokeStyle = cc(line.noteSelected);
        ctx.lineWidth = 1;
        roundRectPath(ctx, x - 0.5, y - 0.5, nw + 1, nh + 1, radius.xs);
        ctx.stroke();
      }
    }
  }

  // Overlays.
  const ghostRect = (g: { start: number; length: number; pitch: number }) => {
    const x = Math.round(g.start * pxPerBeat) + 1;
    const nw = Math.max(3, Math.round(g.length * pxPerBeat) - 2);
    return { x, y: yOfPitch(geo, g.pitch) + 0.5, nw, nh: rowH - 1 };
  };
  if (overlay.pencil) {
    const { x, y, nw, nh } = ghostRect(overlay.pencil);
    ctx.fillStyle = cc(fill.pencilPreview);
    roundRectPath(ctx, x, y, nw, nh, radius.xs);
    ctx.fill();
    ctx.strokeStyle = cc(color.accent);
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  if (overlay.ghost) {
    const { x, y, nw, nh } = ghostRect(overlay.ghost);
    ctx.fillStyle = cc(fill.dragGhost);
    roundRectPath(ctx, x, y, nw, nh, radius.xs);
    ctx.fill();
    ctx.strokeStyle = cc(line.dragGhostEdge);
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  // Playhead relative to the clip start.
  const posBars = beatsToBars(
    state.transport.positionBeats,
    state.transport.timeSignature,
  );
  const px = Math.round((posBars - startBar) * ppb);
  if (px >= 0 && px <= w) {
    withShadows(ctx, canvasShadow.playheadRuler, () => {
      ctx.fillStyle = cc(color.accent);
      ctx.fillRect(px, 0, 1, h);
    });
  }
}

/** Diatonic steps above C for each pitch class (sharps sit on the natural below). */
const DIATONIC = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];

/** Read-only notation view: a treble staff with note heads by pitch and beat. */
function drawScore(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  geo: RollGeometry,
): void {
  const { clip, ppb, pxPerBeat, gridTop } = geo;
  const gap = timeline.staffLineGap;
  const staffTop = gridTop + Math.round((h - gridTop) / 2) - gap * 2;
  // Staff lines: E4 G4 B4 D5 F5 → bottom line is E4 (pitch 64), diatonic index of E4 = 4*7+2 = 30.
  const bottomLineStep = 4 * 7 + 2;
  const yOfStep = (step: number) =>
    staffTop + gap * 4 - ((step - bottomLineStep) * gap) / 2;

  ctx.fillStyle = cc(line.staff);
  for (let i = 0; i < 5; i++)
    ctx.fillRect(0, Math.round(staffTop + i * gap), w, 1);

  ctx.font = monoFont("small");
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  const startBar = clip?.startBar ?? 0;
  const bars = Math.ceil(w / ppb) + 1;
  for (let b = 0; b < bars; b++) {
    const x = Math.round(b * ppb);
    ctx.fillStyle = cc(line.rulerBar);
    ctx.fillRect(x, 0, 1, gridTop);
    ctx.fillStyle = color.ink300;
    ctx.fillText(String(startBar + b + 1), x + 5, 3);
    ctx.fillStyle = cc(line.staff);
    ctx.fillRect(x, Math.round(staffTop), 1, gap * 4 + 1);
  }

  ctx.font = uiFont("small", "semibold");
  ctx.fillStyle = color.ink500;
  ctx.fillText(
    `${state.transport.timeSignature.numerator}/${state.transport.timeSignature.denominator} · ${state.transport.key} · treble`,
    6,
    gridTop + 6,
  );

  if (!clip || clip.data.kind !== "midi") return;
  for (const n of clip.data.notes) {
    const step = Math.floor(n.pitch / 12) * 7 + DIATONIC[n.pitch % 12]!;
    const sharp = isBlackKey(n.pitch);
    const x = Math.round(n.start * pxPerBeat) + timeline.noteHeadR + 2;
    const y = yOfStep(step);
    if (y < gridTop + 4 || y > h - 4) continue;
    // Ledger lines outside the staff.
    ctx.fillStyle = cc(line.staff);
    if (step < bottomLineStep)
      for (let s = bottomLineStep - 2; s >= step; s -= 2)
        ctx.fillRect(x - 7, Math.round(yOfStep(s)), 14, 1);
    if (step > bottomLineStep + 8)
      for (let s = bottomLineStep + 10; s <= step; s += 2)
        ctx.fillRect(x - 7, Math.round(yOfStep(s)), 14, 1);
    ctx.fillStyle = n.agent ? cc(color.accent) : cc(line.noteHead);
    ctx.beginPath();
    ctx.ellipse(
      x,
      y,
      timeline.noteHeadR + 1,
      timeline.noteHeadR,
      -0.35,
      0,
      Math.PI * 2,
    );
    ctx.fill();
    // Stem, up for low notes and down for high ones.
    const stemUp = step < bottomLineStep + 4;
    ctx.fillRect(
      stemUp ? x + timeline.noteHeadR : x - timeline.noteHeadR - 1,
      stemUp ? y - gap * 3 : y,
      1,
      gap * 3,
    );
    if (sharp) {
      ctx.font = monoFont("small");
      ctx.fillText("#", x - 14, y - 5);
    }
    // Duration tail for notes longer than a beat.
    if (n.length > 1) {
      ctx.fillStyle = cc(line.staff);
      ctx.fillRect(
        x + timeline.noteHeadR + 2,
        Math.round(y),
        Math.round((n.length - 1) * pxPerBeat) - 4,
        1,
      );
    }
  }
}
