/**
 * Deterministic audio for the demo session's "generated" sources. Same seed,
 * same sound, so the mock session is stable across launches. Pure DSP over a
 * Float32Array; no Web Audio needed, so it also runs offline.
 */
import type { WaveKind } from '@ondera/core';

function lcg(seed: number): () => number {
  let s = (seed * 7919 + 13) >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

const TEMPO = 120;
const BEAT = 60 / TEMPO;

function drums(out: Float32Array, sr: number, rnd: () => number): void {
  const n = out.length;
  const beats = Math.ceil(n / sr / BEAT);
  const variation = rnd();
  for (let b = 0; b < beats; b++) {
    const t0 = b * BEAT;
    const inBar = b % 4;
    // Kick on 1 and 3 (and a ghost on the "and" of 4 sometimes).
    if (inBar === 0 || inBar === 2) kick(out, sr, t0);
    if (inBar === 3 && variation > 0.5) kick(out, sr, t0 + BEAT / 2, 0.5);
    // Snare on 2 and 4.
    if (inBar === 1 || inBar === 3) snare(out, sr, t0, rnd);
    // Eighth-note hats, accented off-beats.
    hat(out, sr, t0, 0.35, rnd);
    hat(out, sr, t0 + BEAT / 2, 0.22, rnd);
  }
}

function kick(out: Float32Array, sr: number, t0: number, gain = 0.9): void {
  const start = Math.floor(t0 * sr);
  const len = Math.floor(0.35 * sr);
  let phase = 0;
  for (let i = 0; i < len && start + i < out.length; i++) {
    const t = i / sr;
    const f = 150 * Math.exp(-t * 18) + 45;
    phase += (2 * Math.PI * f) / sr;
    const env = Math.exp(-t * 9);
    out[start + i]! += Math.sin(phase) * env * gain;
  }
}

function snare(out: Float32Array, sr: number, t0: number, rnd: () => number): void {
  const start = Math.floor(t0 * sr);
  const len = Math.floor(0.22 * sr);
  let phase = 0;
  for (let i = 0; i < len && start + i < out.length; i++) {
    const t = i / sr;
    phase += (2 * Math.PI * 190) / sr;
    const tone = Math.sin(phase) * Math.exp(-t * 30) * 0.5;
    const noise = (rnd() * 2 - 1) * Math.exp(-t * 16) * 0.6;
    out[start + i]! += (tone + noise) * 0.8;
  }
}

function hat(out: Float32Array, sr: number, t0: number, gain: number, rnd: () => number): void {
  const start = Math.floor(t0 * sr);
  const len = Math.floor(0.06 * sr);
  let hp = 0;
  for (let i = 0; i < len && start + i < out.length; i++) {
    const t = i / sr;
    const white = rnd() * 2 - 1;
    // crude one-pole high-pass
    hp = 0.6 * (hp + white) - 0.6 * white * 0.2;
    out[start + i]! += (white - hp) * Math.exp(-t * 60) * gain;
  }
}

/** Slow pad following i – VI – iv – v in C minor, one chord per two bars. */
function tonal(out: Float32Array, sr: number, seed: number, rnd: () => number): void {
  const chords = [
    [48, 55, 60, 63], // Cm
    [44, 51, 56, 60], // Ab
    [41, 48, 53, 56], // Fm
    [43, 50, 55, 58], // Gm
  ];
  const detune = 0.3 + rnd() * 0.4;
  const barLen = BEAT * 4;
  const n = out.length;
  const voices = chords[0]!.length;
  const phases = new Float64Array(voices * 2);
  for (let i = 0; i < n; i++) {
    const t = i / sr;
    const chord = chords[Math.floor(t / (barLen * 2)) % chords.length]!;
    const posInChord = (t % (barLen * 2)) / (barLen * 2);
    const env = Math.min(1, posInChord * 6) * Math.min(1, (1 - posInChord) * 8);
    let s = 0;
    for (let v = 0; v < voices; v++) {
      const f = 440 * Math.pow(2, (chord[v]! - 69 + (seed % 3) * 0) / 12);
      phases[v * 2] = (phases[v * 2]! + f / sr) % 1;
      phases[v * 2 + 1] = (phases[v * 2 + 1]! + (f * (1 + detune / 100)) / sr) % 1;
      // soft saw via a few harmonics
      const a = phases[v * 2]!;
      const b = phases[v * 2 + 1]!;
      s += (saw(a) + saw(b)) * 0.5;
    }
    const tremolo = 0.85 + 0.15 * Math.sin(2 * Math.PI * 0.4 * t);
    out[i] = (s / voices) * env * tremolo * 0.35;
  }
}

function saw(phase: number): number {
  // band-limited-ish saw: sum of 6 harmonics
  let s = 0;
  for (let k = 1; k <= 6; k++) s += Math.sin(2 * Math.PI * k * phase) / k;
  return s * 0.6;
}

export function generateBuffer(ctx: BaseAudioContext, seed: number, kind: WaveKind, durationSeconds: number): AudioBuffer {
  const sr = ctx.sampleRate;
  const frames = Math.max(1, Math.round(durationSeconds * sr));
  const buffer = ctx.createBuffer(2, frames, sr);
  const mono = new Float32Array(frames);
  const rnd = lcg(seed);
  if (kind === 'drums') drums(mono, sr, rnd);
  else tonal(mono, sr, seed, rnd);
  // soft-clip and spread
  const l = buffer.getChannelData(0);
  const r = buffer.getChannelData(1);
  for (let i = 0; i < frames; i++) {
    const s = Math.tanh(mono[i]! * 1.2);
    l[i] = s;
    r[i] = s;
  }
  return buffer;
}
