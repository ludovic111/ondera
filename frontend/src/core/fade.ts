import type { AudioClipData, FadeCurve } from "./types";

/**
 * Audio clip fades and gain, mirroring `engine/src/model.rs` (`FadeCurve::gain`,
 * `clamp_fades`) so the arrangement draws what the renderer plays.
 */
export const FADE_CURVES: readonly FadeCurve[] = [
  "equalPower",
  "linear",
  "exponential",
];
export const FADE_CURVE_LABELS: Record<FadeCurve, string> = {
  equalPower: "Equal power",
  linear: "Linear",
  exponential: "Exponential",
};
export const CLIP_GAIN_MIN_DB = -60;
export const CLIP_GAIN_MAX_DB = 24;

/** Gain 0-1 at `x` of the way through a fade-in. Fade-outs mirror it. */
export function fadeGain(curve: FadeCurve, x: number): number {
  const t = Math.min(1, Math.max(0, x));
  switch (curve) {
    case "linear":
      return t;
    case "exponential":
      return (Math.exp(4 * t) - 1) / (Math.exp(4) - 1);
    default:
      return Math.sin((t * Math.PI) / 2);
  }
}

/** Keep fades inside a clip of `length` seconds, shrinking both alike when they overlap. */
export function clampFades(
  fadeIn: number,
  fadeOut: number,
  length: number,
): [number, number] {
  const len = Math.max(0, length);
  const a = Math.min(len, Math.max(0, fadeIn));
  const b = Math.min(len, Math.max(0, fadeOut));
  const sum = a + b;
  return sum > len && sum > 0 ? [(a * len) / sum, (b * len) / sum] : [a, b];
}

export const dbToGain = (db: number) => Math.pow(10, db / 20);

/** The fade and gain values of an audio clip, with the file's defaults filled in. */
export function clipEnvelope(data: AudioClipData) {
  return {
    fadeIn: data.fadeInSeconds ?? 0,
    fadeOut: data.fadeOutSeconds ?? 0,
    curve: data.fadeCurve ?? ("equalPower" as FadeCurve),
    gainDb: data.gainDb ?? 0,
  };
}

/** Gain of a clip `age` seconds in, with `left` seconds to go: fades times clip gain. */
export function envelopeAt(
  env: ReturnType<typeof clipEnvelope>,
  age: number,
  left: number,
): number {
  let g = dbToGain(env.gainDb);
  if (env.fadeIn > 0 && age < env.fadeIn)
    g *= fadeGain(env.curve, age / env.fadeIn);
  if (env.fadeOut > 0 && left < env.fadeOut)
    g *= fadeGain(env.curve, left / env.fadeOut);
  return g;
}
