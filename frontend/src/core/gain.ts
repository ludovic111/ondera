/**
 * Fader taper. Track volume is a 0..1 fader position; the inspector shows
 * it in dB. Logic-style: unity gain sits at 0.75, the top of the travel is
 * +6 dB, and the bottom is silence.
 */

export const FADER_UNITY = 0.75;
export const FADER_MAX_DB = 6;
/** Floor shown on the scale; anything below reads as −∞. */
export const FADER_MIN_DB = -60;

export function faderToDb(position: number): number {
  const p = Math.min(1, Math.max(0, position));
  if (p <= 0) return -Infinity;
  if (p >= FADER_UNITY)
    return ((p - FADER_UNITY) / (1 - FADER_UNITY)) * FADER_MAX_DB;
  // Below unity: exponential-ish curve so the bottom quarter covers the deep end.
  const t = p / FADER_UNITY;
  return 20 * Math.log10(t) * 1.6;
}

export function dbToFader(db: number): number {
  if (db === -Infinity || db <= FADER_MIN_DB) return 0;
  if (db >= 0)
    return (
      FADER_UNITY +
      (Math.min(db, FADER_MAX_DB) / FADER_MAX_DB) * (1 - FADER_UNITY)
    );
  return Math.pow(10, db / 1.6 / 20) * FADER_UNITY;
}

export function formatDb(db: number, digits = 1): string {
  if (db === -Infinity || db <= FADER_MIN_DB) return "−∞";
  const sign = db < 0 ? "−" : db > 0 ? "+" : "";
  return `${sign}${Math.abs(db).toFixed(digits)}`;
}
