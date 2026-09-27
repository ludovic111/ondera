import type { ChannelStrip, Track, TrackKind } from "./types";

/** Sends a strip holds at most; the first two feed A and B unless pointed elsewhere. */
export const MAX_SENDS = 4;

/** The name of what a send feeds: A · Reverb, B · Delay or a bus track. */
export function sendTargetName(
  tracks: readonly Pick<Track, "id" | "name">[],
  bus: string | undefined,
): string {
  if (bus === "bus-a") return "A · Reverb";
  if (bus === "bus-b") return "B · Delay";
  return tracks.find((t) => t.id === bus)?.name ?? bus ?? "—";
}

/** Where a track's fader goes, by name. */
export function outputName(
  tracks: readonly Pick<Track, "id" | "name">[],
  track: Pick<Track, "output">,
): string {
  return (
    (track.output && tracks.find((t) => t.id === track.output)?.name) ??
    "Stereo Out"
  );
}

/** The channel strip a fresh track gets. Also what the inspector shows for tracks without one. */
export function defaultStrip(kind: TrackKind): ChannelStrip {
  return {
    instrument: kind === "midi" ? "Ondera Synth" : "—",
    input: kind === "midi" ? "All MIDI" : kind === "bus" ? "—" : "Input 1",
    output: "Stereo Out",
    inserts: [
      { name: "Empty slot", meta: "", state: "empty" },
      { name: "Empty slot", meta: "", state: "empty" },
      { name: "Empty slot", meta: "", state: "empty" },
      { name: "Empty slot", meta: "", state: "empty" },
    ],
    sends: [
      { name: "A · Reverb", levelDb: -Infinity, bus: "bus-a" },
      { name: "B · Delay", levelDb: -Infinity, bus: "bus-b" },
    ],
  };
}
