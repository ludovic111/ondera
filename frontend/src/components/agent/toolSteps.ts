/**
 * Tool calls in plain words. The agent's commands are the registry's (`track.add`,
 * `clip.setNotes`…); a musician should read what happened, not an API log.
 */
type Args = Record<string, unknown>;
const q = (v: unknown) => (typeof v === "string" && v ? ` “${v}”` : "");
const count = (v: unknown, noun: string) =>
  Array.isArray(v) ? ` ${v.length} ${noun}${v.length === 1 ? "" : "s"}` : "";

const PHRASES: Record<string, (a: Args) => string> = {
  "track.add": (a) =>
    `Added ${a.kind === "audio" ? "an audio" : "an instrument"} track${q(a.name)}`,
  "track.remove": () => "Removed a track",
  "track.rename": (a) => `Renamed a track to${q(a.name)}`,
  "track.setVolume": () => "Set a track's volume",
  "track.setPan": () => "Set a track's pan",
  "track.setMute": (a) =>
    a.mute === false ? "Unmuted a track" : "Muted a track",
  "track.setSolo": (a) =>
    a.solo === false ? "Unsoloed a track" : "Soloed a track",
  "track.duplicate": () => "Duplicated a track",
  "clip.create": (a) => `Created a region${q(a.name)}${count(a.notes, "note")}`,
  "clip.setNotes": (a) => `Wrote${count(a.notes, "note") || " notes"}`,
  "clip.move": () => "Moved a region",
  "clip.resize": () => "Resized a region",
  "clip.trim": () => "Trimmed a region",
  "clip.split": () => "Split a region",
  "clip.duplicate": () => "Duplicated a region",
  "clip.remove": () => "Deleted a region",
  "clip.addLoop": (a) => `Added the loop${q(a.name)}`,
  "clip.quantize": () => "Quantized a region",
  "clip.transpose": (a) => `Transposed by ${a.semitones} semitones`,
  "clip.humanize": () => "Humanized the timing",
  "clip.fitScale": () => "Fitted the notes to the scale",
  "clip.setFades": () => "Shaped a region's fades",
  "clip.setGain": (a) => `Set a region's gain to ${a.gainDb} dB`,
  "marker.add": (a) => `Marked a section${q(a.name)}`,
  "marker.rename": (a) => `Renamed a section to${q(a.name)}`,
  "marker.move": () => "Moved a section marker",
  "marker.remove": () => "Removed a section marker",
  "marker.goto": (a) => `Went to${q(a.name) || " a marker"}`,
  "marker.cycleSection": () => "Looped a section",
  "note.add": () => "Added a note",
  "note.update": () => "Changed a note",
  "note.remove": () => "Removed a note",
  "rhythm.create": () => "Built a rhythm",
  "strip.setPlugin": (a) =>
    `Loaded ${String(a.pluginId ?? "a plugin").replace(/^[a-z0-9]+:/, "")}`,
  "strip.setInstrument": (a) => `Chose the instrument${q(a.instrument)}`,
  "strip.setInsert": (a) =>
    a.effect ? `Inserted${q(a.effect)}` : "Cleared an insert",
  "strip.setParameter": () => "Turned a dial",
  "strip.setParameters": () => "Dialled in a sound",
  "strip.setSendLevel": () => "Set a send level",
  "strip.setBypass": (a) =>
    a.bypassed ? "Bypassed an effect" : "Enabled an effect",
  "preset.load": (a) => `Loaded the preset${q(a.name)}`,
  "master.setVolume": () => "Set the master volume",
  "transport.setTempo": (a) => `Set the tempo to ${a.bpm} bpm`,
  "transport.setKey": (a) => `Set the key to ${a.key}`,
  "transport.setTimeSignature": (a) =>
    `Set the metre to ${a.numerator}/${a.denominator}`,
  "transport.setCycle": () => "Set the cycle range",
  "transport.play": () => "Started playback",
  "transport.stop": () => "Stopped playback",
  "automation.create": () => "Added an automation lane",
  "automation.setPoints": () => "Drew automation",
  "session.batch": (a) =>
    `Made${count(a.commands, "edit") || " several edits"} in one step`,
  "session.importAudio": () => "Imported audio",
  "session.importMidi": () => "Imported a MIDI file",
  "session.exportAudio": () => "Exported the mix",
  "session.exportStems": () => "Exported stems",
  "session.save": () => "Saved the session",
  "take.create": (a) => `Saved the take${q(a.name)}`,
  "ui.screenshot": () => "Looked at the window",
};
const LOOKING =
  /\.(list|get|info|inspect|catalog|commands|status|parameters|describe|folders|devices|peaks|snapshots|providers|transcript|changes)$/;

export interface ToolCall {
  name: string;
  args: Record<string, unknown>;
  ok: boolean;
  result: unknown;
}

/** One line for a tool call. Reads (`*.list`, `*.get`…) are described as looking. */
export function describeTool(tool: ToolCall): string {
  const name = tool.name.replace("_", ".");
  const phrase = PHRASES[name];
  if (phrase) return phrase(tool.args ?? {});
  if (LOOKING.test(name)) {
    const subject = name.split(".")[0] ?? "";
    const SUBJECTS: Record<string, string> = {
      session: "the project",
      track: "the tracks",
      clip: "the regions",
      note: "the notes",
      strip: "the channel",
      plugin: "the plugin library",
      preset: "the presets",
      automation: "the automation",
      view: "the view",
      source: "the audio",
      history: "the undo history",
      marker: "the song sections",
    };
    return `Looked at ${SUBJECTS[subject] ?? subject}`;
  }
  return name;
}
export const isReading = (tool: ToolCall) =>
  LOOKING.test(tool.name.replace("_", ".")) || tool.name.startsWith("ui");

/** Why a step failed, as the host said it. */
export function toolError(tool: ToolCall): string {
  const result = tool.result as { error?: unknown } | null;
  return tool.ok ? "" : String(result?.error ?? "This step did not work");
}
