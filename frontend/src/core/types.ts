/**
 * Session model. Plain data, JSON-serialisable, no methods.
 * Time is expressed in beats (quarter notes) unless a field says otherwise.
 * Bars are 0-based internally; the UI adds 1 for display.
 */

export type TrackId = string;
export type ClipId = string;

export type TrackKind = "audio" | "midi";

/** auto: while armed and not playing back the track's own clip. */
export type Monitor = "off" | "auto" | "on";

export interface Track {
  id: TrackId;
  name: string;
  kind: TrackKind;
  /** Track colour as an oklch() CSS string from the track palette. */
  color: string;
  /** 0..1 linear fader position. */
  volume: number;
  /** -100 (hard left) .. 100 (hard right). */
  pan: number;
  mute: boolean;
  solo: boolean;
  armed: boolean;
  /** Hear the live input through this audio track; absent means off. */
  monitor?: Monitor;
  /** True while an agent is actively editing this track. */
  agentActive: boolean;
}

export interface Note {
  id: string;
  /** Start in beats, relative to the clip start. */
  start: number;
  /** Length in beats. */
  length: number;
  /** MIDI pitch 0..127. */
  pitch: number;
  /** Velocity 1..127. */
  velocity: number;
  /** True when the note was written by an agent. */
  agent?: boolean;
  /** MIDI channel 0..15 it plays on; absent means 0 (channel 1). */
  channel?: number;
}

export type WaveKind = "drums" | "tonal";

/**
 * An audio source: a decoded file, a recording, or a procedurally generated
 * buffer. Core holds only metadata; the host (the app) owns the samples.
 */
export interface AudioSource {
  id: string;
  name: string;
  durationSeconds: number;
  sampleRate: number;
  channels: number;
  /** generated: synthesised by the host from `seed`/`waveKind`; file: imported; recording: captured. */
  origin: "generated" | "file" | "recording";
  seed?: number;
  waveKind?: WaveKind;
  /** File name inside the session bundle, for file and recording sources. */
  fileName?: string;
}

/** equalPower keeps loudness through a crossfade; exponential starts slow. */
export type FadeCurve = "equalPower" | "linear" | "exponential";

export interface AudioClipData {
  kind: "audio";
  sourceId: string;
  /** Where in the source this clip starts, in seconds. */
  offsetSeconds: number;
  /** Fade lengths in seconds of audio; absent means none. */
  fadeInSeconds?: number;
  fadeOutSeconds?: number;
  /** Absent means equalPower. */
  fadeCurve?: FadeCurve;
  /** Clip gain in dB, -60 to +24; absent means 0. */
  gainDb?: number;
}

export type ControllerKind = "cc" | "bend" | "pressure";

/**
 * A controller point in a MIDI clip. The value holds until the next point of
 * the same lane (kind and number), as MIDI does.
 */
export interface Controller {
  id: string;
  kind: ControllerKind;
  /** Controller number, for `cc` only. */
  number?: number;
  /** Beats from the clip start. */
  time: number;
  /** 0..127 for cc and pressure; -8192..8191 for bend. */
  value: number;
  /** True when the point was written by an agent. */
  agent?: boolean;
  /** MIDI channel 0..15; absent means 0. The same controller on two channels is two lanes. */
  channel?: number;
}

export interface MidiClipData {
  kind: "midi";
  notes: Note[];
  /** Absent when the clip has none. */
  controllers?: Controller[];
}

export interface Clip {
  id: ClipId;
  trackId: TrackId;
  name: string;
  /** Start position in bars (0-based). */
  startBar: number;
  /** Length in bars. */
  lengthBars: number;
  data: AudioClipData | MidiClipData;
  /** True when the clip is currently being edited by an agent. */
  agent: boolean;
}

/** A named position on the ruler: where a song section starts. */
export interface Marker {
  id: string;
  /** Zero-based bar. */
  bar: number;
  name: string;
  /** CSS colour; absent uses the theme's marker colour. */
  color?: string | null;
}

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

export interface Transport {
  playing: boolean;
  recording: boolean;
  /** The count-in click is running; the song starts when it ends. */
  countingIn?: boolean;
  /** Playhead position in beats. */
  positionBeats: number;
  tempo: number;
  timeSignature: TimeSignature;
  key: string;
  cycle: boolean;
  /** Cycle range in bars, 0-based, end exclusive. */
  cycleStartBar: number;
  cycleEndBar: number;
  metronome: boolean;
  /** Snap grid as a note division, e.g. 16 for 1/16. */
  snapDivision: number;
}

export type BrowserTab = "instruments" | "loops" | "plugins" | "files";
export type EditorMode = "pianoRoll" | "score" | "step";
export type ArrangeTool = "pointer" | "pencil" | "scissors" | "grid";

export interface View {
  editorLowPitch?: number;
  /** Horizontal zoom of the arrangement, in CSS pixels per bar. */
  pixelsPerBar: number;
  /** Horizontal scroll of the arrangement, in bars (fractional). */
  scrollBars: number;
  agentPanelOpen: boolean;
  selectedTrackId: TrackId | null;
  selectedClipId: ClipId | null;
  /** Clip shown in the bottom editor pane. */
  editorClipId: ClipId | null;
  /** Selected note in the editor, by note id. */
  selectedNoteId: string | null;
  editorMode: EditorMode;
  browserTab: BrowserTab;
  /** Highlighted browser item, by name. */
  browserSelection: string | null;
  arrangeTool: ArrangeTool;
  /** Keep the playhead on screen while playing. */
  followPlayhead: boolean;
}

export type AgentStatus = "idle" | "working";

export interface AgentAction {
  description: string;
  /** Short form for the chip that floats over the lane, e.g. "adding 7ths, bars 5–8". */
  summary: string;
  /** Progress 0..1. */
  progress: number;
  progressLabel: string;
}

export interface AgentLogEntry {
  id: string;
  title: string;
  /** The equivalent CLI invocation, shown as detail. */
  detail: string;
  /** Track colour or neutral. */
  color: string;
  live: boolean;
  reverted: boolean;
}

export interface AgentState {
  status: AgentStatus;
  transport: string;
  current: AgentAction | null;
  log: AgentLogEntry[];
  draft: string;
}

export interface BrowserItem {
  id?: string;
  name: string;
  meta: string;
  color: string | null;
  /** Plugins only: starred, and the sound folder it is filed under. */
  favorite?: boolean;
  folder?: string;
}

export interface BrowserGroup {
  name: string;
  items: BrowserItem[];
  /** Plugin tabs: a sound folder, or one of the two shortcuts above them. */
  kind?: "folder" | "favorites" | "recent";
  color?: string;
}

export interface InsertSlot {
  name: string;
  meta: string;
  state: "active" | "bypassed" | "empty";
}

export interface Send {
  name: string;
  /** -100 .. 0 dB, -Infinity for off. */
  levelDb: number;
}

export interface ChannelStrip {
  /** Instrument preset name for MIDI tracks; '—' on audio tracks. */
  instrument: string;
  input: string;
  output: string;
  inserts: InsertSlot[];
  sends: Send[];
}

export interface AudioSettings {
  sampleRate: number;
  bitDepth: number;
  bufferSize: number;
}

/** Fake meter readings. 0..1 linear per side. Mock in Phase 1. */
export interface Meters {
  masterL: number;
  masterR: number;
  cpu: number;
  channelL: number;
  channelR: number;
  /** Microphone level while an audio track is armed or recording, 0-1. */
  input?: number;
}

export interface Session {
  name: string;
  audio: AudioSettings;
  tracks: Track[];
  clips: Clip[];
  /** Song sections in bar order. */
  markers: Marker[];
  /** Audio sources referenced by audio clips, keyed by id. */
  sources: Record<string, AudioSource>;
  transport: Transport;
  view: View;
  agent: AgentState;
  /** Browser sidebar content, one list per tab. */
  browser: Record<BrowserTab, BrowserGroup[]>;
  /** Channel strips keyed by track id. Tracks without one get defaultStrip(). */
  strips: Record<TrackId, ChannelStrip>;
  meters: Meters;
}
