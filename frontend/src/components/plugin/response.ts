/**
 * What each stock plugin does to a signal, as curves for its display. The maths
 * mirrors sdk/src/dsp.rs and engine/src/stock.rs so the picture is the sound:
 * same RBJ biquads for the EQ, same SVF damping for the filter, same soft knee
 * for the dynamics.
 */

const RATE = 48_000;
export const F_MIN = 20;
export const F_MAX = 20_000;

/** x in 0..1 → Hz on a log axis, and back. */
export const xToHz = (x: number) => F_MIN * Math.pow(F_MAX / F_MIN, x);
export const hzToX = (hz: number) =>
  Math.log(Math.max(F_MIN, Math.min(F_MAX, hz)) / F_MIN) /
  Math.log(F_MAX / F_MIN);

type Biquad = [
  b0: number,
  b1: number,
  b2: number,
  a0: number,
  a1: number,
  a2: number,
];

function shelf(hz: number, gainDb: number, high: boolean): Biquad {
  const a = Math.pow(10, gainDb / 40);
  const w = (2 * Math.PI * Math.min(Math.max(hz, 10), RATE * 0.45)) / RATE;
  const s = Math.sin(w);
  const c = Math.cos(w);
  const sq = 2 * Math.sqrt(a) * ((s / 2) * Math.SQRT2);
  const k = high ? -1 : 1;
  return [
    a * (a + 1 - k * (a - 1) * c + sq),
    k * 2 * a * (a - 1 - k * (a + 1) * c),
    a * (a + 1 - k * (a - 1) * c - sq),
    a + 1 + k * (a - 1) * c + sq,
    k * -2 * (a - 1 + k * (a + 1) * c),
    a + 1 + k * (a - 1) * c - sq,
  ];
}

function peaking(hz: number, gainDb: number, q: number): Biquad {
  const a = Math.pow(10, gainDb / 40);
  const w = (2 * Math.PI * Math.min(Math.max(hz, 10), RATE * 0.45)) / RATE;
  const alpha = Math.sin(w) / (2 * Math.max(q, 0.05));
  const c = Math.cos(w);
  return [
    1 + alpha * a,
    -2 * c,
    1 - alpha * a,
    1 + alpha / a,
    -2 * c,
    1 - alpha / a,
  ];
}

function magnitudeDb([b0, b1, b2, a0, a1, a2]: Biquad, hz: number): number {
  const w = (2 * Math.PI * hz) / RATE;
  const c1 = Math.cos(w);
  const c2 = Math.cos(2 * w);
  const s1 = Math.sin(w);
  const s2 = Math.sin(2 * w);
  const num = (b0 + b1 * c1 + b2 * c2) ** 2 + (b1 * s1 + b2 * s2) ** 2;
  const den = (a0 + a1 * c1 + a2 * c2) ** 2 + (a1 * s1 + a2 * s2) ** 2;
  return 10 * Math.log10(num / den);
}

export interface EqBands {
  lowGain: number;
  lowFreq: number;
  midGain: number;
  midFreq: number;
  midQ: number;
  highGain: number;
  highFreq: number;
}

/** Summed response of the three Channel EQ bands at `hz`, in dB. */
export function eqDb(b: EqBands, hz: number): number {
  return (
    magnitudeDb(shelf(b.lowFreq, b.lowGain, false), hz) +
    magnitudeDb(peaking(b.midFreq, b.midGain, b.midQ), hz) +
    magnitudeDb(shelf(b.highFreq, b.highGain, true), hz)
  );
}

/** State-variable filter response; `type` 0 low-pass, 1 high-pass, 2 band-pass. */
export function filterDb(
  type: number,
  cutoff: number,
  resonancePercent: number,
  hz: number,
): number {
  const k = 2 - 1.9 * Math.min(1, Math.max(0, resonancePercent / 100));
  const f = hz / cutoff;
  // |1 / (1 - f² + jkf)| with the numerator of each tap.
  const den = Math.sqrt((1 - f * f) ** 2 + (k * f) ** 2);
  const num = type === 1 ? f * f : type === 2 ? k * f : 1;
  return 20 * Math.log10(Math.max(1e-6, num / den));
}

/** One-pole tone control used by the saturators and the echo. */
export const toneDb = (cutoff: number, hz: number) =>
  -10 * Math.log10(1 + (hz / cutoff) ** 2);

/** Compressor output level for an input level, both dBFS. Soft 6 dB knee. */
export function compressDb(
  input: number,
  threshold: number,
  ratio: number,
  makeup = 0,
): number {
  const knee = 6;
  const over = input - threshold;
  const slope = 1 - 1 / Math.max(1, ratio);
  let reduction = 0;
  if (over > knee / 2) reduction = over * slope;
  else if (over > -knee / 2)
    reduction = (slope * (over + knee / 2) ** 2) / (2 * knee);
  return input - reduction + makeup;
}

/** Gate output level: below threshold the signal drops by `range` dB. */
export const gateDb = (input: number, threshold: number, range: number) =>
  input >= threshold ? input : Math.max(-96, input + range);

export const limitDb = (input: number, gain: number, ceiling: number) =>
  Math.min(input + gain, ceiling);

/** Waveshaper transfer, x and result in -1..1. */
export function saturate(x: number, driveDb: number, hard: boolean): number {
  const g = Math.pow(10, driveDb / 20);
  const y = hard
    ? Math.max(-1, Math.min(1, (x * g) / (1 + Math.abs(x * g) * 0.35)))
    : Math.tanh(x * g);
  return y / (hard ? 1 : Math.max(1e-6, Math.tanh(g)));
}

/** A sine after bit reduction and sample-and-hold, t in 0..1. */
export function crush(t: number, bits: number, downsample: number): number {
  const steps = Math.max(4, Math.round(96 / downsample));
  const held = Math.floor(t * steps) / steps;
  const levels = Math.pow(2, bits - 1);
  return Math.round(Math.sin(held * 2 * Math.PI * 2) * levels) / levels;
}

/** LFO value in -1..1 at phase 0..1; shape 0 sine, 1 triangle, 2 square. */
export function lfo(shape: number, phase: number): number {
  const p = ((phase % 1) + 1) % 1;
  if (shape === 1) return 1 - 4 * Math.abs(p - 0.5);
  if (shape === 2) return p < 0.5 ? 1 : -1;
  return Math.sin(p * 2 * Math.PI);
}

export interface Adsr {
  attack: number;
  decay: number;
  sustain: number;
  release: number;
}

/**
 * Envelope as polyline points in 0..1 × 0..1. Stage widths are compressed with
 * a square root so a 5 ms attack is still visible beside a 3 s release.
 */
export function envelopePoints(e: Adsr, hold = 0.22): [number, number][] {
  const w = (ms: number) => Math.sqrt(Math.max(1, ms));
  const a = w(e.attack);
  const d = w(e.decay);
  const r = w(e.release);
  const total = (a + d + r) / (1 - hold);
  const s = Math.min(1, Math.max(0, e.sustain));
  const x1 = a / total;
  const x2 = x1 + d / total;
  const x3 = x2 + hold;
  return [
    [0, 0],
    [x1, 1],
    [x2, s],
    [x3, s],
    [1, 0],
  ];
}

/** Sample `fn` over 0..1 into an SVG path in a w × h box; fn returns 0..1 up. */
export function plot(
  fn: (x: number) => number,
  w: number,
  h: number,
  samples = 96,
): string {
  let d = "";
  for (let i = 0; i <= samples; i++) {
    const x = i / samples;
    const y = Math.min(1, Math.max(0, fn(x)));
    d += `${i ? "L" : "M"}${(x * w).toFixed(1)} ${((1 - y) * h).toFixed(1)}`;
  }
  return d;
}
