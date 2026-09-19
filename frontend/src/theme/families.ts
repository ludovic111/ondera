/**
 * Which sound family a browser folder belongs to. Folder names come from the engine
 * (`plugin.folders`); a folder the user made up gets a stable family from its name.
 */
import type { FamilyKey } from "./schema";
import { FAMILY_HUES } from "./schema";

const BY_FOLDER: Record<string, FamilyKey> = {
  Synths: "famSynth",
  Keys: "famKeys",
  Bass: "famBass",
  Drums: "famDrums",
  Pads: "famPad",
  Samplers: "famSampler",
  Textures: "famTexture",
  "Other Instruments": "famSynth",
  Dynamics: "famDynamics",
  "EQ & Filter": "famEq",
  Distortion: "famDrive",
  Modulation: "famMod",
  "Space & Time": "famSpace",
  Pitch: "famPitch",
  "Channel Strips": "famKeys",
  Mastering: "famSampler",
  Restoration: "famTexture",
  Utility: "famUtility",
  "Other Effects": "famUtility",
};
const KEYS = Object.keys(FAMILY_HUES) as FamilyKey[];

export function familyOf(folder: string): FamilyKey {
  const known = BY_FOLDER[folder];
  if (known) return known;
  let hash = 0;
  for (const ch of folder) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return KEYS[hash % KEYS.length]!;
}

/** CSS custom property holding the family colour, e.g. `var(--color-fam-space)`. */
export const familyVar = (folder: string): string =>
  `var(--color-${familyOf(folder).replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)})`;
