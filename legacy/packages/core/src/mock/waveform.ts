import type { WaveKind } from '../model/types';

/** Peaks per bar in the mock waveform cache. 256 keeps 5× zoom smooth. */
export const PEAKS_PER_BAR = 256;

function lcg(seed: number): () => number {
  let s = (seed * 7919 + 13) >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/**
 * Deterministic fake waveform: one amplitude (0..1) per peak slot.
 * Same seed → same picture, so the UI is stable across reloads.
 */
export function generatePeaks(seed: number, kind: WaveKind, lengthBars: number, beatsPerBar = 4): Float32Array {
  const n = Math.max(1, Math.round(lengthBars * PEAKS_PER_BAR));
  const rnd = lcg(seed);
  const out = new Float32Array(n);
  const peaksPerBeat = PEAKS_PER_BAR / beatsPerBar;
  for (let i = 0; i < n; i++) {
    const beatPhase = (i % peaksPerBeat) / peaksPerBeat;
    const x = i / (PEAKS_PER_BAR / 48); // equivalent pixel at default zoom, matches the design
    const env =
      kind === 'drums'
        ? beatPhase < 0.18
          ? 1 - beatPhase * 2
          : 0.22 + 0.12 * Math.sin(x / 3)
        : 0.42 + 0.3 * Math.sin(x / 41 + (seed % 7)) + 0.14 * Math.sin(x / 9);
    out[i] = Math.min(1, Math.max(0.02, (rnd() * 0.55 + 0.45) * env * 0.92));
  }
  return out;
}

const cache = new Map<string, Float32Array>();

export function getPeaks(seed: number, kind: WaveKind, lengthBars: number): Float32Array {
  const key = `${seed}:${kind}:${lengthBars}`;
  let peaks = cache.get(key);
  if (!peaks) {
    peaks = generatePeaks(seed, kind, lengthBars);
    cache.set(key, peaks);
  }
  return peaks;
}
