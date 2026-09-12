/**
 * Session model. Plain data, JSON-serialisable, no methods.
 * Time is expressed in beats (quarter notes) unless a field says otherwise.
 * Bars are 0-based internally; the UI adds 1 for display.
 */

export type TrackId = string;
export type ClipId = string;

export type TrackKind = 'audio' | 'midi';

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
  /** True while an agent is actively editing this track. */
  agentActive: boolean;
}

export interface Note {
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
}

export type WaveKind = 'drums' | 'tonal';

export interface AudioClipData {
  kind: 'audio';
  /** Seed for the deterministic mock waveform generator. */
  waveSeed: number;
  waveKind: WaveKind;
}

export interface MidiClipData {
  kind: 'midi';
  notes: Note[];
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

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

export interface Transport {
  playing: boolean;
  recording: boolean;
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

export type BrowserTab = 'instruments' | 'loops' | 'plugins' | 'files';
export type EditorMode = 'pianoRoll' | 'score' | 'step';
export type ArrangeTool = 'pointer' | 'pencil' | 'scissors' | 'grid';

export interface View {
  /** Horizontal zoom of the arrangement, in CSS pixels per bar. */
  pixelsPerBar: number;
  /** Horizontal scroll of the arrangement, in bars (fractional). */
  scrollBars: number;
  agentPanelOpen: boolean;
  selectedTrackId: TrackId | null;
  selectedClipId: ClipId | null;
  /** Clip shown in the bottom editor pane. */
  editorClipId: ClipId | null;
  editorMode: EditorMode;
  browserTab: BrowserTab;
  arrangeTool: ArrangeTool;
}

export type AgentStatus = 'idle' | 'working';

export interface AgentAction {
  description: string;
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
  name: string;
  meta: string;
  color: string | null;
  highlighted: boolean;
}

export interface BrowserGroup {
  name: string;
  items: BrowserItem[];
}

export interface InsertSlot {
  name: string;
  meta: string;
  state: 'active' | 'bypassed' | 'empty';
}

export interface Send {
  name: string;
  /** -100 .. 0 dB, -Infinity for off. */
  levelDb: number;
}

export interface ChannelStrip {
  instrument: string;
  input: string;
  output: string;
  inserts: InsertSlot[];
  sends: Send[];
  /** Fader in dB. */
  volumeDb: number;
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
}

export interface Session {
  name: string;
  audio: AudioSettings;
  tracks: Track[];
  clips: Clip[];
  transport: Transport;
  view: View;
  agent: AgentState;
  browser: BrowserGroup[];
  /** Channel strip for the selected track. Keyed by track id. */
  strips: Record<TrackId, ChannelStrip>;
  meters: Meters;
}
