/** Track palette from the design spec sheet: L 0.72–0.78, C 0.12–0.14. */
export const TRACK_PALETTE = {
  drums: "oklch(0.72 0.14 40)",
  bass: "oklch(0.72 0.13 300)",
  keys: "oklch(0.75 0.13 250)",
  pad: "oklch(0.75 0.12 330)",
  vox: "oklch(0.78 0.14 85)",
  bgv: "oklch(0.75 0.12 130)",
  guitar: "oklch(0.72 0.13 20)",
  riser: "oklch(0.75 0.14 60)",
} as const;
