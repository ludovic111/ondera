import type { TimeSignature } from './types';

/** Ticks per quarter note. Logic-style: 240 ticks per sixteenth. */
export const PPQ = 960;

export interface BarBeatPosition {
  /** 1-based bar. */
  bar: number;
  /** 1-based beat within the bar. */
  beat: number;
  /** 1-based sixteenth within the beat. */
  division: number;
  /** 0..239 ticks within the sixteenth. */
  tick: number;
}

export function beatsPerBar(sig: TimeSignature): number {
  return sig.numerator * (4 / sig.denominator);
}

export function beatsToBars(beats: number, sig: TimeSignature): number {
  return beats / beatsPerBar(sig);
}

export function barsToBeats(bars: number, sig: TimeSignature): number {
  return bars * beatsPerBar(sig);
}

export function beatsToBarBeat(beats: number, sig: TimeSignature): BarBeatPosition {
  const bpb = beatsPerBar(sig);
  const safe = Math.max(0, beats);
  const bar = Math.floor(safe / bpb);
  const beatInBar = safe - bar * bpb;
  const beat = Math.floor(beatInBar);
  const frac = beatInBar - beat;
  const sixteenth = frac * 4;
  const division = Math.floor(sixteenth);
  const tick = Math.floor((sixteenth - division) * (PPQ / 4));
  return { bar: bar + 1, beat: beat + 1, division: division + 1, tick };
}

export function beatsToSeconds(beats: number, tempo: number): number {
  return (beats * 60) / tempo;
}

export interface SmpteTime {
  hours: number;
  minutes: number;
  seconds: number;
  frames: number;
}

export function secondsToSmpte(seconds: number, fps = 30): SmpteTime {
  const safe = Math.max(0, seconds);
  const whole = Math.floor(safe);
  const frames = Math.floor((safe - whole) * fps);
  return {
    hours: Math.floor(whole / 3600),
    minutes: Math.floor((whole % 3600) / 60),
    seconds: whole % 60,
    frames,
  };
}

const pad = (n: number, w: number) => String(n).padStart(w, '0');

export function formatSmpte(t: SmpteTime): string {
  return `${pad(t.hours, 2)}:${pad(t.minutes, 2)}:${pad(t.seconds, 2)}:${pad(t.frames, 2)}`;
}

export function formatBarBeat(p: BarBeatPosition): string {
  return `${pad(p.bar, 3)}.${p.beat}.${p.division}.${pad(p.tick, 3)}`;
}

/** Short form used on the playhead flag, e.g. "5.2.3". */
export function formatBarBeatShort(p: BarBeatPosition): string {
  return `${p.bar}.${p.beat}.${p.division}`;
}

/** Length of one snap step in bars for a note division (16 = sixteenth). */
export function snapStepBars(division: number, sig: TimeSignature): number {
  // A whole note is 4 beats; a 1/division note is 4/division beats.
  return 4 / division / beatsPerBar(sig);
}

/** Round a bar position to the snap grid. */
export function snapBars(bar: number, division: number, sig: TimeSignature): number {
  const step = snapStepBars(division, sig);
  return Math.round(bar / step) * step;
}

/** Round a beat position (clip-relative) to the snap grid. */
export function snapBeats(beats: number, division: number): number {
  const step = 4 / division;
  return Math.round(beats / step) * step;
}

export function barsToSeconds(bars: number, tempo: number, sig: TimeSignature): number {
  return beatsToSeconds(barsToBeats(bars, sig), tempo);
}

export function secondsToBeats(seconds: number, tempo: number): number {
  return (seconds * tempo) / 60;
}

export function secondsToBars(seconds: number, tempo: number, sig: TimeSignature): number {
  return beatsToBars(secondsToBeats(seconds, tempo), sig);
}
