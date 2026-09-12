import type { AudioSource, BrowserGroup, BrowserTab, Clip, Note, Session, Track, WaveKind } from '../model/types';
import { barsToSeconds } from '../model/time';
import { defaultStrip } from '../model/strip';
import { TRACK_PALETTE } from '../model/palette';
import { NEUTRAL_LOG_COLOR as NEUTRAL } from '../commands/agent';

function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** Sparse random notes for arrangement-level MIDI clips (the design's dashes). */
function scatterNotes(seed: number, lengthBars: number, agent: boolean): Note[] {
  const rnd = lcg(seed);
  const count = lengthBars * 5;
  const notes: Note[] = [];
  for (let i = 0; i < count; i++) {
    const note: Note = {
      id: `n${seed}-${i}`,
      start: rnd() * lengthBars * 4,
      length: 0.25 + rnd() * 0.75,
      pitch: 36 + Math.floor(rnd() * 36),
      velocity: 60 + Math.floor(rnd() * 60),
    };
    if (agent && i % 3 === 0) note.agent = true;
    notes.push(note);
  }
  return notes;
}

/** The "Bass verse" pattern from the design: root, octave, fifth, root, third per bar. */
function bassVerseNotes(lengthBars: number): Note[] {
  const roots = [0, 0, 8, 8, 5, 5, 7, 7];
  const thirds = [3, 3, 4, 4, 3, 3, 4, 4];
  const base = 36; // C2
  const notes: Note[] = [];
  for (let bar = 0; bar < lengthBars; bar++) {
    const r = roots[bar % roots.length]!;
    const third = thirds[bar % thirds.length]!;
    const x = bar * 4;
    const pattern: [number, number, number, number][] = [
      [0, 1.5, r, 110],
      [1.5, 0.5, r + 12, 80],
      [2, 1, r + 7, 100],
      [3, 0.75, r, 110],
      [3.5, 0.5, r + third, 70],
    ];
    pattern.forEach(([st, len, semis, vel], i) => {
      notes.push({ id: `bv${bar}-${i}`, start: x + st, length: len, pitch: base + semis, velocity: vel });
    });
  }
  return notes;
}

const tracks: Track[] = [
  { id: 'drums', name: 'Drums', kind: 'audio', color: TRACK_PALETTE.drums, volume: 0.78, pan: 0, mute: false, solo: false, armed: false, agentActive: false },
  { id: 'bass', name: 'Bass', kind: 'midi', color: TRACK_PALETTE.bass, volume: 0.7, pan: -8, mute: false, solo: false, armed: false, agentActive: false },
  { id: 'keys', name: 'Keys', kind: 'midi', color: TRACK_PALETTE.keys, volume: 0.62, pan: 12, mute: false, solo: false, armed: false, agentActive: true },
  { id: 'pad', name: 'Pad', kind: 'midi', color: TRACK_PALETTE.pad, volume: 0.5, pan: 0, mute: false, solo: false, armed: false, agentActive: false },
  { id: 'vox', name: 'Lead Vox', kind: 'audio', color: TRACK_PALETTE.vox, volume: 0.82, pan: 0, mute: false, solo: false, armed: true, agentActive: false },
  { id: 'bgv', name: 'BGV', kind: 'audio', color: TRACK_PALETTE.bgv, volume: 0.55, pan: -30, mute: true, solo: false, armed: false, agentActive: false },
  { id: 'guitar', name: 'Guitar', kind: 'audio', color: TRACK_PALETTE.guitar, volume: 0.66, pan: 35, mute: false, solo: false, armed: false, agentActive: false },
  { id: 'riser', name: 'Riser FX', kind: 'midi', color: TRACK_PALETTE.riser, volume: 0.6, pan: 0, mute: false, solo: false, armed: false, agentActive: false },
];

const MOCK_TEMPO = 120;
const MOCK_SIG = { numerator: 4, denominator: 4 };

const sources: Record<string, AudioSource> = {};

/** A generated source lasting exactly `lengthBars` at the mock tempo. */
function source(id: string, name: string, lengthBars: number, seed: number, waveKind: WaveKind): string {
  sources[id] = {
    id,
    name,
    durationSeconds: barsToSeconds(lengthBars, MOCK_TEMPO, MOCK_SIG),
    sampleRate: 48000,
    channels: 2,
    origin: 'generated',
    seed,
    waveKind,
  };
  return id;
}

const audio = (id: string, trackId: string, name: string, startBar: number, lengthBars: number, waveSeed: number, waveKind: WaveKind = 'tonal'): Clip => ({
  id,
  trackId,
  name,
  startBar,
  lengthBars,
  agent: false,
  data: { kind: 'audio', sourceId: source(`src-${id}`, name, lengthBars, waveSeed, waveKind), offsetSeconds: 0 },
});

const midi = (id: string, trackId: string, name: string, startBar: number, lengthBars: number, notes: Note[], agent = false): Clip => ({
  id, trackId, name, startBar, lengthBars, agent, data: { kind: 'midi', notes },
});

const clips: Clip[] = [
  audio('drums-1', 'drums', 'Drums_take3', 0, 12, 1, 'drums'),
  midi('bass-1', 'bass', 'Bass intro', 0, 4, scatterNotes(11, 4, false)),
  midi('bass-2', 'bass', 'Bass verse', 4, 8, bassVerseNotes(8)),
  midi('keys-1', 'keys', 'Keys A', 0, 4, scatterNotes(21, 4, false)),
  midi('keys-2', 'keys', 'Keys B', 4, 4, scatterNotes(22, 4, true), true),
  midi('keys-3', 'keys', 'Keys B', 8, 4, scatterNotes(23, 4, false)),
  midi('pad-1', 'pad', 'Pad swell', 4, 8, scatterNotes(31, 8, false)),
  audio('vox-1', 'vox', 'LV_v2_comp', 4, 4, 2),
  audio('vox-2', 'vox', 'LV_v2_comp', 8, 4, 3),
  audio('bgv-1', 'bgv', 'BGV stack', 8, 4, 4),
  audio('guitar-1', 'guitar', 'Gtr DI', 2, 6, 5),
  audio('guitar-2', 'guitar', 'Gtr DI', 8, 3, 6),
  midi('riser-1', 'riser', 'Riser', 7, 1, scatterNotes(41, 1, false)),
];


const item = (name: string, meta = '', color: string | null = null) => ({ name, meta, color });

const browser: Record<BrowserTab, BrowserGroup[]> = {
  instruments: [
    {
      name: 'Ondera',
      items: [
        item('Ondera Synth', '', TRACK_PALETTE.bass),
        item('E-Piano Mk I', '', TRACK_PALETTE.keys),
        item('Drum Machine', '', TRACK_PALETTE.drums),
        item('Sampler', '', TRACK_PALETTE.vox),
      ],
    },
    {
      name: 'Installed',
      items: [item('Vital', 'VST3'), item('Surge XT', 'CLAP'), item('Dexed', 'VST3'), item('Odin 2', 'VST3'), item('Helm', 'LV2')],
    },
    {
      name: 'Recent',
      items: [
        item('Sub Bass 808', 'preset', TRACK_PALETTE.bass),
        item('Glass Keys', 'preset', TRACK_PALETTE.keys),
        item('Choir Pad', 'preset', TRACK_PALETTE.pad),
      ],
    },
  ],
  loops: [
    {
      name: 'Drums',
      items: [item('Boom Bap 92', '92 bpm', TRACK_PALETTE.drums), item('Four Floor 124', '124 bpm', TRACK_PALETTE.drums), item('Brushes Swing', '108 bpm', TRACK_PALETTE.drums)],
    },
    {
      name: 'Melodic',
      items: [item('Rhodes Comp Cm', 'C min', TRACK_PALETTE.keys), item('Analog Pad Swell', 'C min', TRACK_PALETTE.pad), item('Bass Pluck 120', '120 bpm', TRACK_PALETTE.bass)],
    },
    {
      name: 'FX',
      items: [item('Riser 1 bar', '1 bar', TRACK_PALETTE.riser), item('Reverse Cymbal', '2 bars', TRACK_PALETTE.riser), item('Vinyl Crackle', 'loop')],
    },
  ],
  plugins: [
    {
      name: 'Ondera',
      items: [item('Ondera Comp', 'dynamics'), item('Channel EQ', 'eq'), item('Tape Sat', 'saturation'), item('Chorus', 'modulation'), item('Space', 'reverb'), item('Echo', 'delay')],
    },
    {
      name: 'Installed',
      items: [item('Pro-Q 3', 'VST3'), item('Valhalla Room', 'VST3'), item('Airwindows Console', 'CLAP'), item('LSP Limiter', 'LV2')],
    },
  ],
  files: [
    {
      name: 'Project',
      items: [item('Nightfall.ondera', 'today'), item('Bounces', '3 files'), item('Audio Files', '14 files')],
    },
    {
      name: 'Recent',
      items: [item('Drums_take3.wav', '48k · 24'), item('LV_v2_comp.wav', '48k · 24'), item('Gtr DI.wav', '48k · 24'), item('BGV stack.wav', '48k · 24')],
    },
  ],
};

export function createMockSession(): Session {
  return {
    name: 'Nightfall.ondera',
    audio: { sampleRate: 48000, bitDepth: 24, bufferSize: 128 },
    tracks,
    clips,
    sources,
    transport: {
      playing: false,
      recording: false,
      // 005.2.3.120 in the design: bar 5, beat 2, sixteenth 3, tick 120
      positionBeats: 16 + 1 + 0.5 + 120 / 960,
      tempo: MOCK_TEMPO,
      timeSignature: MOCK_SIG,
      key: 'C min',
      cycle: true,
      cycleStartBar: 4,
      cycleEndBar: 8,
      metronome: false,
      snapDivision: 16,
    },
    view: {
      pixelsPerBar: 48,
      scrollBars: 0,
      agentPanelOpen: true,
      selectedTrackId: 'bass',
      selectedClipId: 'bass-2',
      editorClipId: 'bass-2',
      selectedNoteId: null,
      editorMode: 'pianoRoll',
      browserTab: 'instruments',
      browserSelection: 'E-Piano Mk I',
      arrangeTool: 'pointer',
      followPlayhead: true,
    },
    agent: {
      status: 'working',
      transport: 'via MCP',
      current: {
        description:
          'Thickening the Keys part in bars 5–8: adding a 7th to each chord and lowering velocities 8% so it sits under the vocal.',
        summary: 'adding 7ths, bars 5–8',
        progress: 18 / 29,
        progressLabel: '18 / 29 notes',
      },
      log: [
        { id: 'l1', title: 'Adding 7ths to Keys B, bars 5–8', detail: 'keys/clip:2 · 18 of 29 notes · in progress', color: TRACK_PALETTE.keys, live: true, reverted: false },
        { id: 'l2', title: 'Quantized Keys B to 1/16, 62% strength', detail: 'ondera clip quantize keys:2 --grid 1/16 --strength .62', color: TRACK_PALETTE.keys, live: false, reverted: false },
        { id: 'l3', title: 'Created send Keys → A · Reverb, −12 dB', detail: 'ondera send add keys --bus A --level -12', color: TRACK_PALETTE.keys, live: false, reverted: false },
        { id: 'l4', title: 'Trimmed Guitar clip 1 end to bar 8', detail: 'ondera clip trim guitar:1 --end 8.1.1', color: TRACK_PALETTE.guitar, live: false, reverted: false },
        { id: 'l5', title: 'Renamed “Audio 5” to “Lead Vox”', detail: 'ondera track rename 5 "Lead Vox"', color: TRACK_PALETTE.vox, live: false, reverted: false },
        { id: 'l6', title: 'Set project key to C minor', detail: 'ondera project set key Cm', color: NEUTRAL, live: false, reverted: false },
      ],
      draft: '',
    },
    browser,
    strips: {
      keys: { ...defaultStrip('midi'), instrument: 'E-Piano Mk I' },
      pad: { ...defaultStrip('midi'), instrument: 'Choir Pad' },
      riser: { ...defaultStrip('midi'), instrument: 'Riser' },
      bass: {
        instrument: 'Ondera Synth',
        input: 'All MIDI',
        output: 'Stereo Out',
        inserts: [
          { name: 'Ondera Comp', meta: '3.2 dB GR', state: 'active' },
          { name: 'Tape Sat', meta: 'Drive 24%', state: 'active' },
          { name: 'Chorus', meta: 'bypassed', state: 'bypassed' },
          { name: 'Empty slot', meta: '', state: 'empty' },
        ],
        sends: [
          { name: 'A · Reverb', levelDb: -12 },
          { name: 'B · Delay', levelDb: -Infinity },
        ],
      },
    },
    meters: { masterL: 16 / 22, masterR: 15 / 22, cpu: 0.34, channelL: 13 / 20, channelR: 12 / 20 },
  };
}
