import {
  barsToSeconds,
  beatsPerBar,
  beatsToBars,
  type Clip,
  type Session,
  type Track,
} from "@ondera/core";
import {
  canvasShadow,
  clipMix,
  color,
  fill,
  line,
  radius,
  size,
  white,
} from "../theme/tokens";
import { mix, withAlpha } from "../theme/color";
import {
  cc,
  hline,
  roundRectPath,
  uiFont,
  monoFont,
  withShadows,
} from "./paint";
import { drawWaveform } from "./waveform";
import { library } from "../audio/library";

export interface LaneGeometry {
  /** Pixels per bar. */
  ppb: number;
  /** First visible bar (fractional). */
  scrollBars: number;
  rowHeight: number;
}

/** Ephemeral drawing state from an interaction in progress. Never session state. */
export interface LaneOverlay {
  /** Outline where a dragged clip would land. */
  ghost?: { trackIndex: number; startBar: number; lengthBars: number };
  /** Clip being drawn with the pencil. */
  pencil?: { trackIndex: number; startBar: number; lengthBars: number };
  /** Scissors guide: where the cut would fall. */
  split?: { trackIndex: number; bar: number };
  /** Lane a dragged file would drop onto. */
  dropTrackIndex?: number;
}

export function barToX(bar: number, geo: LaneGeometry): number {
  return (bar - geo.scrollBars) * geo.ppb;
}

export function xToBar(x: number, geo: LaneGeometry): number {
  return geo.scrollBars + x / geo.ppb;
}

export function laneGeometry(state: Session): LaneGeometry {
  return {
    ppb: state.view.pixelsPerBar,
    scrollBars: state.view.scrollBars,
    rowHeight: size.trackRow,
  };
}

/** Returns the clip under (x, y) in lane pixels, or null. */
export function hitTestClip(state: Session, x: number, y: number): Clip | null {
  const geo = laneGeometry(state);
  const row = Math.floor(y / geo.rowHeight);
  const track = state.tracks[row];
  if (!track) return null;
  const bar = xToBar(x, geo);
  // Later clips draw on top, so hit-test back to front.
  for (let i = state.clips.length - 1; i >= 0; i--) {
    const clip = state.clips[i]!;
    if (clip.trackId !== track.id) continue;
    if (bar >= clip.startBar && bar < clip.startBar + clip.lengthBars)
      return clip;
  }
  return null;
}

/** Which edge of `clip` (if any) is within the grip zone of x. */
export function clipEdgeAt(
  state: Session,
  clip: Clip,
  x: number,
): "start" | "end" | null {
  const geo = laneGeometry(state);
  const x0 = barToX(clip.startBar, geo);
  const x1 = barToX(clip.startBar + clip.lengthBars, geo);
  const grip = Math.min(size.clipEdgeGrip, (x1 - x0) / 3);
  if (x - x0 <= grip) return "start";
  if (x1 - x <= grip) return "end";
  return null;
}

export function drawLanes(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  state: Session,
  overlay: LaneOverlay = {},
  visibleTop = 0,
): void {
  const geo = laneGeometry(state);
  const { tracks, clips, transport, view } = state;
  const rowH = geo.rowHeight;
  const tracksBottom = tracks.length * rowH;

  // Empty area below the last track.
  ctx.fillStyle = color.timelineEmpty;
  ctx.fillRect(0, tracksBottom, w, Math.max(0, h - tracksBottom));

  // Lane backgrounds, one per track.
  tracks.forEach((track, i) => {
    const y = i * rowH;
    if (y + rowH < visibleTop || y > h) return;
    const selected = view.selectedTrackId === track.id;
    ctx.fillStyle = selected
      ? color.timelineSelected
      : track.agentActive
        ? color.timelineAgent
        : color.timeline;
    ctx.fillRect(0, y, w, rowH);
    if (overlay.dropTrackIndex === i) {
      ctx.fillStyle = cc(fill.dropTarget);
      ctx.fillRect(0, y, w, rowH);
    }
    hline(ctx, 0, y, w, line.laneTop);
    hline(ctx, 0, y + rowH - 1, w, line.laneBottom);
  });

  // Cycle range tint.
  if (transport.cycle) {
    const x0 = barToX(transport.cycleStartBar, geo);
    const x1 = barToX(transport.cycleEndBar, geo);
    ctx.fillStyle = cc(fill.cycleLane);
    ctx.fillRect(x0, 0, x1 - x0, tracksBottom);
  }

  // Grid.
  drawGrid(ctx, w, tracksBottom, geo);

  // Clips.
  tracks.forEach((track, i) => {
    const y = i * rowH;
    if (y + rowH < visibleTop || y > h) return;
    for (const clip of clips) {
      if (clip.trackId !== track.id) continue;
      const x = barToX(clip.startBar, geo) + 1;
      const cw = clip.lengthBars * geo.ppb - 2;
      if (x + cw < 0 || x > w) continue;
      drawClip(
        ctx,
        clip,
        track,
        x,
        y + size.clipInset,
        cw,
        rowH - size.clipInset * 2,
        view.selectedClipId === clip.id,
        geo,
        state,
      );
    }
  });

  // Interaction overlays.
  if (overlay.pencil) {
    const p = overlay.pencil;
    const x = barToX(p.startBar, geo);
    ctx.fillStyle = cc(fill.pencilPreview);
    roundRectPath(
      ctx,
      x + 1,
      p.trackIndex * rowH + size.clipInset,
      Math.max(2, p.lengthBars * geo.ppb - 2),
      rowH - size.clipInset * 2,
      radius.clip,
    );
    ctx.fill();
    ctx.strokeStyle = cc(color.accent);
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  if (overlay.ghost) {
    const g = overlay.ghost;
    const x = barToX(g.startBar, geo);
    ctx.fillStyle = cc(fill.dragGhost);
    roundRectPath(
      ctx,
      x + 1.5,
      g.trackIndex * rowH + size.clipInset + 0.5,
      Math.max(2, g.lengthBars * geo.ppb - 3),
      rowH - size.clipInset * 2 - 1,
      radius.clip,
    );
    ctx.fill();
    ctx.strokeStyle = cc(line.dragGhostEdge);
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  if (overlay.split) {
    const x = Math.round(barToX(overlay.split.bar, geo));
    ctx.fillStyle = cc(line.splitGuide);
    ctx.fillRect(x, overlay.split.trackIndex * rowH, 1, rowH);
  }

  // Playhead.
  const px = Math.round(
    barToX(beatsToBars(transport.positionBeats, transport.timeSignature), geo),
  );
  if (px >= 0 && px <= w) {
    withShadows(ctx, canvasShadow.playhead, () => {
      ctx.fillStyle = cc(color.accent);
      ctx.fillRect(px, 0, 1, h);
    });
  }
}

function drawGrid(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  geo: LaneGeometry,
): void {
  const firstBar = Math.floor(geo.scrollBars);
  const lastBar = Math.ceil(geo.scrollBars + w / geo.ppb);
  const showBeats = geo.ppb >= 24;
  for (let bar = firstBar; bar <= lastBar; bar++) {
    const x = Math.round(barToX(bar, geo));
    ctx.fillStyle = cc(line.barLine);
    ctx.fillRect(x, 0, 1, h);
    if (showBeats) {
      ctx.fillStyle = cc(line.beatLine);
      for (let b = 1; b < 4; b++)
        ctx.fillRect(Math.round(x + (b * geo.ppb) / 4), 0, 1, h);
    }
  }
}

function drawClip(
  ctx: CanvasRenderingContext2D,
  clip: Clip,
  track: Track,
  x: number,
  y: number,
  w: number,
  h: number,
  selected: boolean,
  geo: LaneGeometry,
  state: Session,
): void {
  const r = radius.clip;
  const faceTop = mix(track.color, color.panel, clipMix.faceTop);
  const faceBottom = mix(track.color, color.panel, clipMix.faceBottom);

  // Drop shadow: two soft layers, light from above.
  withShadows(ctx, canvasShadow.clipDrop, () => {
    ctx.fillStyle = faceBottom;
    roundRectPath(ctx, x, y, w, h, r);
    ctx.fill();
  });

  // Face gradient.
  const grad = ctx.createLinearGradient(0, y, 0, y + h);
  grad.addColorStop(0, faceTop);
  grad.addColorStop(1, faceBottom);
  ctx.fillStyle = grad;
  roundRectPath(ctx, x, y, w, h, r);
  ctx.fill();

  ctx.save();
  roundRectPath(ctx, x, y, w, h, r);
  ctx.clip();

  // Title strip.
  const titleH = size.clipTitle;
  ctx.fillStyle = cc(fill.clipTitle);
  ctx.fillRect(x, y, w, titleH);
  hline(ctx, x, y + titleH - 1, w, line.clipTitleBottom);

  // Specular highlight and contact line.
  hline(ctx, x, y, w, white(0.22));
  hline(ctx, x, y + h - 1, w, withAlpha(color.desk, 0.4));

  // Name.
  ctx.font = uiFont("small", "semibold");
  ctx.textBaseline = "middle";
  ctx.textAlign = "left";
  ctx.fillStyle = cc(line.clipName);
  ctx.fillText(clip.name, x + 6, y + titleH / 2 + 0.5, Math.max(0, w - 12));
  if (clip.agent && w > 70) {
    ctx.font = monoFont("kind", "medium");
    ctx.textAlign = "right";
    ctx.fillStyle = cc(color.accent);
    ctx.fillText("AGENT", x + w - 6, y + titleH / 2 + 0.5);
    ctx.textAlign = "left";
  }

  // Content.
  const cy = y + titleH;
  const ch = h - titleH;
  if (clip.data.kind === "audio") {
    drawAudioContent(ctx, clip, x, cy, w, ch, state);
  } else {
    drawMidiPreview(ctx, clip.data.notes, x, cy, w, ch, geo, state);
  }
  ctx.restore();

  // Rings.
  if (clip.agent) {
    withShadows(ctx, canvasShadow.agentRing, () => {
      ctx.strokeStyle = cc(color.accent);
      ctx.lineWidth = 1;
      roundRectPath(ctx, x + 0.5, y + 0.5, w - 1, h - 1, r);
      ctx.stroke();
    });
  } else if (selected) {
    ctx.strokeStyle = white(0.85);
    ctx.lineWidth = 1.5;
    roundRectPath(ctx, x - 0.25, y - 0.25, w + 0.5, h + 0.5, r);
    ctx.stroke();
  }
}

/** Draw peaks from the Rust audio library; never synthesize a visual waveform. */
function drawAudioContent(
  ctx: CanvasRenderingContext2D,
  clip: Clip,
  x: number,
  y: number,
  w: number,
  h: number,
  state: Session,
): void {
  if (clip.data.kind !== "audio") return;
  const { tempo, timeSignature } = state.transport;
  const clipSeconds = barsToSeconds(clip.lengthBars, tempo, timeSignature);
  const visX0 = Math.max(x, 0);
  const visX1 = Math.min(x + w, ctx.canvas.clientWidth);
  if (visX1 <= visX0) return;
  const f0 = (visX0 - x) / w;
  const f1 = (visX1 - x) / w;

  const real = library.peaksFor(clip.data.sourceId);
  if (real) {
    const first =
      (clip.data.offsetSeconds + f0 * clipSeconds) *
      library.rateFor(clip.data.sourceId);
    const last =
      (clip.data.offsetSeconds + f1 * clipSeconds) *
      library.rateFor(clip.data.sourceId);
    drawWaveform(ctx, real, visX0, y, visX1 - visX0, h, first, last);
    return;
  }
  // Keep the lane empty until the Rust decoder supplies actual peaks.
}

function drawMidiPreview(
  ctx: CanvasRenderingContext2D,
  notes: readonly {
    start: number;
    length: number;
    pitch: number;
    agent?: boolean;
  }[],
  x: number,
  y: number,
  _w: number,
  h: number,
  geo: LaneGeometry,
  state: Session,
): void {
  if (notes.length === 0) return;
  let lo = 127;
  let hi = 0;
  for (const n of notes) {
    if (n.pitch < lo) lo = n.pitch;
    if (n.pitch > hi) hi = n.pitch;
  }
  const range = Math.max(12, hi - lo);
  const pad = 4;
  const pxPerBeat = geo.ppb / beatsPerBar(state.transport.timeSignature);
  const noteH = size.clipNoteH;
  for (const n of notes) {
    const nx = x + n.start * pxPerBeat;
    const nw = Math.max(3, n.length * pxPerBeat);
    const ny = y + pad + (1 - (n.pitch - lo) / range) * (h - pad * 2 - noteH);
    if (n.agent) {
      withShadows(ctx, canvasShadow.agentNote, () => {
        ctx.fillStyle = cc(color.accent);
        ctx.fillRect(nx, ny, nw, noteH);
      });
    } else {
      ctx.fillStyle = cc(line.midiNote);
      ctx.fillRect(nx, ny, nw, noteH);
    }
  }
}
