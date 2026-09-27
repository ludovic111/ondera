import type { TimeSignature } from "./types";
import { beatsPerBar } from "./time";

/**
 * Mirror of `engine/src/tempo.rs`. The song starts at `transport.tempo`; each change takes
 * over at its bar, at once or by a ramp from the tempo before it that is linear in beats.
 * Positions stay bars and beats; this turns them into seconds for the clock, audio clips
 * (whose audio plays at its own speed) and their fades.
 */
export interface TempoPoint {
  /** Zero-based bar, after bar 0. */
  bar: number;
  bpm: number;
  /** Glide from the previous tempo to reach `bpm` at `bar`. */
  ramp?: boolean;
}

export const TEMPO_MIN = 20;
export const TEMPO_MAX = 400;

interface Segment {
  beat: number;
  seconds: number;
  bpm: number;
  /** Tempo change per beat; zero when the tempo holds. */
  slope: number;
}

const secondsTo = (s: Segment, beat: number) => {
  const x = beat - s.beat;
  return s.slope === 0
    ? (x * 60) / s.bpm
    : (60 / s.slope) * Math.log1p((s.slope * x) / s.bpm);
};
const beatsAfter = (s: Segment, seconds: number) =>
  s.slope === 0
    ? (seconds * s.bpm) / 60
    : (s.bpm * Math.expm1((s.slope * seconds) / 60)) / s.slope;

export class TempoMap {
  private readonly segments: Segment[];
  constructor(initial: number, points: readonly TempoPoint[], bpb: number) {
    this.segments = [{ beat: 0, seconds: 0, bpm: initial, slope: 0 }];
    for (const p of points) {
      const beat = p.bar * bpb;
      const last = this.segments[this.segments.length - 1]!;
      if (beat <= last.beat) continue;
      if (p.ramp) last.slope = (p.bpm - last.bpm) / (beat - last.beat);
      this.segments.push({
        beat,
        seconds: last.seconds + secondsTo(last, beat),
        bpm: p.bpm,
        slope: 0,
      });
    }
  }
  get isConstant(): boolean {
    return this.segments.length === 1;
  }
  private at(beat: number): Segment {
    let lo = 0;
    let hi = this.segments.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (this.segments[mid]!.beat <= beat) lo = mid;
      else hi = mid - 1;
    }
    return this.segments[lo]!;
  }
  bpm(beat: number): number {
    const s = this.at(beat);
    return s.bpm + s.slope * (beat - s.beat);
  }
  seconds(beat: number): number {
    const s = this.at(beat);
    return s.seconds + secondsTo(s, beat);
  }
  beat(seconds: number): number {
    let lo = 0;
    let hi = this.segments.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (this.segments[mid]!.seconds <= seconds) lo = mid;
      else hi = mid - 1;
    }
    const s = this.segments[lo]!;
    return s.beat + beatsAfter(s, seconds - s.seconds);
  }
  /** Seconds between two beats. */
  duration(from: number, to: number): number {
    return this.isConstant
      ? ((to - from) * 60) / this.segments[0]!.bpm
      : this.seconds(to) - this.seconds(from);
  }
  /** Beats that `seconds` cover from `from` on. */
  beatsFor(from: number, seconds: number): number {
    return this.isConstant
      ? (seconds * this.segments[0]!.bpm) / 60
      : this.beat(this.seconds(from) + seconds) - from;
  }
}

interface TempoSource {
  transport: { tempo: number; timeSignature: TimeSignature };
  tempoChanges?: readonly TempoPoint[];
}

let cached: {
  tempo: number;
  numerator: number;
  denominator: number;
  points: readonly TempoPoint[] | undefined;
  map: TempoMap;
} | null = null;

/** The song's tempo map; the last one is reused while tempo, meter and changes stay the same. */
export function tempoMap(state: TempoSource): TempoMap {
  const { tempo, timeSignature: sig } = state.transport;
  const points = state.tempoChanges;
  if (
    cached &&
    cached.tempo === tempo &&
    cached.numerator === sig.numerator &&
    cached.denominator === sig.denominator &&
    cached.points === points
  )
    return cached.map;
  const map = new TempoMap(tempo, points ?? [], beatsPerBar(sig));
  cached = {
    tempo,
    numerator: sig.numerator,
    denominator: sig.denominator,
    points,
    map,
  };
  return map;
}

/** Seconds between two bars of the song. */
export function barsSeconds(
  state: TempoSource,
  startBar: number,
  endBar: number,
): number {
  const bpb = beatsPerBar(state.transport.timeSignature);
  return tempoMap(state).duration(startBar * bpb, endBar * bpb);
}

/** The bar reached `seconds` after `startBar`. */
export function barAfterSeconds(
  state: TempoSource,
  startBar: number,
  seconds: number,
): number {
  const bpb = beatsPerBar(state.transport.timeSignature);
  return startBar + tempoMap(state).beatsFor(startBar * bpb, seconds) / bpb;
}

/** The tempo at a beat of the song. */
export function tempoAt(state: TempoSource, beat: number): number {
  return tempoMap(state).bpm(beat);
}

/**
 * Where the tempo in force at `beat` was set: bar 0 for the starting tempo, else the bar of
 * the latest change at or before it. Dragging the tempo readout edits that one.
 */
export function tempoSourceBar(state: TempoSource, beat: number): number {
  const bar = beat / beatsPerBar(state.transport.timeSignature);
  let found = 0;
  for (const p of state.tempoChanges ?? []) if (p.bar <= bar + 1e-9) found = p.bar;
  return found;
}
