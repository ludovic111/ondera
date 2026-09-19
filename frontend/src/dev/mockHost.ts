/**
 * Development-only stand-in for the Rust host. `npm run dev` in a plain browser
 * has no Tauri bridge, so this answers the handful of commands the renderer
 * needs with a fixed song. It exists to check themes and layouts quickly:
 * `?theme=modern|skeuo|aero&mode=dark|light&panel=mixer|settings|plugin:<name>`.
 * Never imported by a production build.
 */
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

type Params = Record<string, unknown>;

const query = new URLSearchParams(location.search);
const panel = query.get("panel") ?? "";

const TRACKS = [
  ["drums", "Drums", "midi", "oklch(0.72 0.14 40)", "Drum Kit"],
  ["bass", "Bass", "midi", "oklch(0.72 0.13 300)", "Sub Bass"],
  ["keys", "Keys", "midi", "oklch(0.75 0.13 250)", "Grand Piano"],
  ["pad", "Choir pad", "midi", "oklch(0.75 0.12 330)", "Choir Pad"],
  ["vox", "Lead vocal", "audio", "oklch(0.78 0.14 85)", "—"],
  ["riser", "Riser", "midi", "oklch(0.75 0.14 60)", "Riser"],
] as const;

function notes(seed: number, bars: number, low: number) {
  const out = [];
  let s = seed;
  const rand = () => (s = (s * 1103515245 + 12345) % 2147483648) / 2147483648;
  for (let i = 0; i < bars * 8; i++) {
    if (rand() < 0.35) continue;
    out.push({
      id: `n${seed}-${i}`,
      start: i / 2,
      length: rand() < 0.3 ? 1 : 0.5,
      pitch: low + [0, 3, 5, 7, 10, 12, 15][Math.floor(rand() * 7)]!,
      velocity: 60 + Math.floor(rand() * 60),
      agent: seed === 3 && i > 40,
    });
  }
  return out;
}

const clips = [
  ["drums", "Beat A", 0, 4, 1, 36],
  ["drums", "Beat B", 4, 8, 2, 36],
  ["bass", "Bassline", 0, 8, 5, 36],
  ["keys", "Chords", 2, 6, 3, 60],
  ["keys", "Chords var.", 8, 4, 4, 60],
  ["pad", "Bloom", 4, 8, 6, 55],
  ["riser", "Lift", 10, 2, 7, 72],
].map(([trackId, name, startBar, lengthBars, seed, low], i) => ({
  id: `c${i}`,
  trackId,
  name,
  startBar,
  lengthBars,
  agent: i === 3,
  data: {
    kind: "midi",
    notes: notes(seed as number, lengthBars as number, low as number),
  },
}));
clips.push({
  id: "cv",
  trackId: "vox",
  name: "Vocal take 3",
  startBar: 4,
  lengthBars: 6,
  agent: false,
  data: { kind: "audio", sourceId: "src1", offsetSeconds: 0 } as never,
});

const strips = Object.fromEntries(
  TRACKS.map(([id, , kind, , instrument], i) => [
    id,
    {
      instrument,
      input: "",
      output: "",
      synth:
        kind === "midi"
          ? { id: `syn-${id}`, name: instrument, plugin: `stock:${instrument}` }
          : undefined,
      inserts: [
        {
          id: `ins-${id}-0`,
          name: "Channel EQ",
          meta: "EQ",
          state: "active",
          plugin: "stock:Channel EQ",
        },
        {
          id: `ins-${id}-1`,
          name: "Ondera Comp",
          meta: "4:1",
          state: i % 2 ? "bypassed" : "active",
          plugin: "stock:Ondera Comp",
        },
      ],
      sends: [{ levelDb: -12 }, { levelDb: -Infinity }],
    },
  ]),
);

const session = {
  name: "Night Drive",
  snapshotSequence: 1,
  masterVolume: 0.8,
  automation: [],
  audio: { sampleRate: 48000, bitDepth: 24, bufferSize: 128 },
  tracks: TRACKS.map(([id, name, kind, color], i) => ({
    id,
    name,
    kind,
    color,
    volume: 0.55 + ((i * 7) % 4) / 10,
    pan: [-20, 0, 15, -35, 0, 40][i],
    mute: i === 5,
    solo: false,
    armed: i === 4,
    monitor: (i === 4 ? "auto" : "off") as "off" | "auto" | "on",
    agentActive: i === 2,
  })),
  clips,
  sources: {
    src1: {
      id: "src1",
      name: "Vocal take 3.wav",
      durationSeconds: 12,
      sampleRate: 48000,
      channels: 2,
      origin: "file",
    },
  },
  strips,
  transport: {
    playing: false,
    recording: false,
    positionBeats: 13,
    tempo: 122,
    timeSignature: { numerator: 4, denominator: 4 },
    key: "C min",
    cycle: true,
    cycleStartBar: 4,
    cycleEndBar: 8,
    metronome: true,
    snapDivision: 16,
  },
  view: {
    pixelsPerBar: 64,
    scrollBars: 0,
    agentPanelOpen: true,
    selectedTrackId: "keys",
    selectedClipId: "c3",
    editorClipId: "c3",
    selectedNoteId: null,
    editorMode: "pianoRoll",
    browserTab: "instruments",
    browserSelection: "Grand Piano",
    arrangeTool: "pointer",
    followPlayhead: true,
  },
  meters: { masterL: 0, masterR: 0, channelL: 0, channelR: 0, cpu: 0 },
  agent: { status: "idle", transport: "", current: null, log: [], draft: "" },
};

const ui = {
  dirty: true,
  path: null,
  history: { canUndo: true, canRedo: false },
  agentPanel: query.get("agent") !== "0",
  automation: false,
  settings: panel === "settings",
  settingsSection: "interface",
  help: false,
  mixer: panel === "mixer",
  export: false,
  recovery: false,
  tool: "pointer",
  musicalTyping: false,
  pluginWindows: [] as string[],
  status: "",
  error: null,
  busy: false,
  prompt: null,
  scale: 1,
  appearance: query.get("theme") ?? "skeuo",
  mode: query.get("mode") ?? "dark",
  recoveryStatus: "",
  update: { available: null, installed: false, busy: false },
};

const settings = {
  general: { confirmBeforeQuit: true, checkUpdatesOnStart: true },
  audio: { outputDevice: null, inputDevice: null },
  interface: {
    appearance: ui.appearance,
    mode: ui.mode,
    scale: 1,
    showTooltips: true,
    followPlayhead: true,
  },
  agent: { provider: "claude" },
};

const P = (
  id: number,
  name: string,
  min: number,
  max: number,
  value: number,
  unit = "",
  labels: string[] = [],
  logarithmic = false,
) => ({
  id,
  name,
  min,
  max,
  value,
  default: value,
  unit,
  steps: labels.length,
  labels,
  logarithmic,
});
const hz = (id: number, name: string, min: number, max: number, v: number) =>
  P(id, name, min, max, v, "Hz", [], true);
const ON_OFF = ["Off", "On"];

/** Mirrors engine/src/stock.rs closely enough to lay out every faceplate. */
export const STOCK_PARAMETERS: Record<string, ReturnType<typeof P>[]> = {
  "Ondera Comp": [
    P(0, "Threshold", -60, 0, -18, "dB"),
    P(1, "Ratio", 1, 20, 4, ":1"),
    P(2, "Attack", 0.1, 100, 10, "ms"),
    P(3, "Release", 10, 1000, 120, "ms"),
    P(4, "Makeup", 0, 24, 3, "dB"),
    P(5, "Mix", 0, 100, 100, "%"),
  ],
  "Channel EQ": [
    P(0, "Low Gain", -15, 15, 1.5, "dB"),
    hz(1, "Low Freq", 40, 500, 120),
    P(2, "Mid Gain", -15, 15, -2, "dB"),
    hz(3, "Mid Freq", 200, 8000, 1000),
    P(4, "Mid Q", 0.3, 5, 0.8),
    P(5, "High Gain", -15, 15, 2, "dB"),
    hz(6, "High Freq", 2000, 16000, 6000),
  ],
  "Tape Sat": [
    P(0, "Drive", 0, 24, 6, "dB"),
    hz(1, "Tone", 1000, 20000, 9000),
    P(2, "Mix", 0, 100, 100, "%"),
    P(3, "Output", -24, 6, -2, "dB"),
  ],
  Overdrive: [
    P(0, "Drive", 0, 40, 12, "dB"),
    hz(1, "Tone", 500, 12000, 4000),
    P(2, "Mix", 0, 100, 100, "%"),
    P(3, "Output", -24, 6, -6, "dB"),
  ],
  Chorus: [
    P(0, "Rate", 0.05, 5, 0.6, "Hz"),
    P(1, "Depth", 0, 100, 40, "%"),
    P(2, "Spread", 0, 100, 50, "%"),
    P(3, "Mix", 0, 100, 50, "%"),
  ],
  Space: [
    P(0, "Size", 0, 100, 55, "%"),
    P(1, "Damp", 0, 100, 40, "%"),
    P(2, "Pre-delay", 0, 100, 10, "ms"),
    P(3, "Width", 0, 100, 100, "%"),
    P(4, "Mix", 0, 100, 30, "%"),
  ],
  Echo: [
    P(0, "Sync", 0, 6, 3, "", [
      "Free",
      "1/16",
      "1/8",
      "1/4",
      "1/8.",
      "1/4.",
      "1/2",
    ]),
    P(1, "Time", 10, 2000, 375, "ms"),
    P(2, "Feedback", 0, 95, 35, "%"),
    hz(3, "Tone", 500, 20000, 6000),
    P(4, "Ping-pong", 0, 1, 1, "", ON_OFF),
    P(5, "Mix", 0, 100, 35, "%"),
  ],
  Gate: [
    P(0, "Threshold", -80, 0, -40, "dB"),
    P(1, "Attack", 0.1, 50, 1, "ms"),
    P(2, "Hold", 0, 500, 50, "ms"),
    P(3, "Release", 5, 1000, 100, "ms"),
    P(4, "Range", -80, 0, -80, "dB"),
  ],
  Limiter: [
    P(0, "Input", 0, 24, 4, "dB"),
    P(1, "Ceiling", -20, 0, -0.3, "dB"),
    P(2, "Release", 10, 1000, 80, "ms"),
  ],
  Filter: [
    P(0, "Type", 0, 2, 0, "", ["Low-pass", "High-pass", "Band-pass"]),
    hz(1, "Cutoff", 20, 20000, 1000),
    P(2, "Resonance", 0, 100, 45, "%"),
    P(3, "Drive", 0, 24, 0, "dB"),
  ],
  Phaser: [
    P(0, "Rate", 0.02, 5, 0.3, "Hz"),
    P(1, "Depth", 0, 100, 70, "%"),
    P(2, "Stages", 0, 5, 2, "", ["2", "4", "6", "8", "10", "12"]),
    P(3, "Feedback", -90, 90, 30, "%"),
    hz(4, "Center", 200, 4000, 800),
    P(5, "Mix", 0, 100, 50, "%"),
  ],
  Tremolo: [
    P(0, "Rate", 0.1, 20, 4, "Hz"),
    P(1, "Depth", 0, 100, 60, "%"),
    P(2, "Shape", 0, 2, 0, "", ["Sine", "Triangle", "Square"]),
    P(3, "Stereo", 0, 180, 90, "°"),
  ],
  Bitcrusher: [
    P(0, "Bits", 2, 16, 6, "bit"),
    P(1, "Downsample", 1, 64, 8, "x"),
    P(2, "Mix", 0, 100, 100, "%"),
  ],
  "Stereo Width": [
    P(0, "Width", 0, 200, 120, "%"),
    P(1, "Bass Mono", 0, 500, 120, "Hz"),
  ],
  Utility: [
    P(0, "Gain", -60, 12, 0, "dB"),
    P(1, "Pan", -100, 100, 0),
    P(2, "Invert L", 0, 1, 0, "", ON_OFF),
    P(3, "Invert R", 0, 1, 0, "", ON_OFF),
    P(4, "Mono", 0, 1, 0, "", ON_OFF),
  ],
  Transient: [
    P(0, "Attack", -100, 100, 30, "%"),
    P(1, "Sustain", -100, 100, -20, "%"),
    P(2, "Output", -24, 6, 0, "dB"),
  ],
  Flanger: [
    P(0, "Rate", 0.02, 8, 0.35, "Hz"),
    P(1, "Depth", 0, 100, 70, "%"),
    P(2, "Manual", 0.2, 10, 2.5, "ms"),
    P(3, "Feedback", -95, 95, 55, "%"),
    P(4, "Mix", 0, 100, 50, "%"),
  ],
  "Auto Pan": [
    P(0, "Rate", 0.05, 12, 1, "Hz"),
    P(1, "Depth", 0, 100, 80, "%"),
    P(2, "Shape", 0, 2, 0, "", ["Sine", "Triangle", "Square"]),
  ],
  "Auto Filter": [
    hz(0, "Cutoff", 40, 16000, 600),
    P(1, "Resonance", 0, 100, 45, "%"),
    P(2, "LFO Rate", 0.02, 12, 0.5, "Hz"),
    P(3, "LFO Depth", 0, 100, 40, "%"),
    P(4, "Envelope", -100, 100, 40, "%"),
  ],
  "De-Esser": [
    hz(0, "Frequency", 2000, 12000, 6500),
    P(1, "Threshold", -60, 0, -28, "dB"),
    P(2, "Range", 0, 24, 9, "dB"),
    P(3, "Listen", 0, 1, 0, "", ON_OFF),
  ],
  "Lo-Fi": [
    P(0, "Wobble", 0, 100, 35, "%"),
    P(1, "Noise", 0, 100, 20, "%"),
    hz(2, "Tone", 800, 16000, 5200),
    P(3, "Crunch", 0, 24, 6, "dB"),
    P(4, "Mix", 0, 100, 100, "%"),
  ],
  "Pitch Shift": [
    P(0, "Semitones", -12, 12, 7, "st"),
    P(1, "Fine", -100, 100, 0, "ct"),
    P(2, "Mix", 0, 100, 50, "%"),
  ],
  Pump: [
    P(0, "Every", 0, 3, 1, "", ["1/2", "1/4", "1/8", "1/16"]),
    P(1, "Depth", 0, 100, 70, "%"),
    P(2, "Recovery", 5, 100, 45, "%"),
    P(3, "Offset", 0, 100, 0, "%"),
  ],
  "Ondera Synth": [
    P(0, "Wave", 0, 2, 0, "", ["Saw", "Square", "Triangle"]),
    hz(1, "Cutoff", 100, 16000, 2400),
    P(2, "Env Amount", 0, 100, 50, "%"),
    P(3, "Attack", 1, 3000, 8, "ms"),
    P(4, "Decay", 5, 3000, 300, "ms"),
    P(5, "Sustain", 0, 100, 60, "%"),
    P(6, "Release", 10, 5000, 400, "ms"),
    P(7, "Level", -24, 6, 0, "dB"),
  ],
  "Choir Pad": [
    hz(0, "Cutoff", 100, 16000, 1800),
    P(1, "Detune", 0, 50, 12, "ct"),
    P(2, "Attack", 1, 3000, 900, "ms"),
    P(3, "Release", 10, 5000, 2200, "ms"),
    P(4, "Level", -24, 6, 0, "dB"),
  ],
  "Grand Piano": [
    P(0, "Attack", 1, 3000, 2, "ms"),
    P(1, "Release", 10, 5000, 600, "ms"),
    P(2, "Level", -24, 6, 0, "dB"),
  ],
  "Drum Kit": [P(0, "Level", -24, 6, 0, "dB")],
  "Analog Bass": [
    hz(0, "Cutoff", 60, 8000, 700),
    P(1, "Env Amount", 0, 100, 40, "%"),
    P(2, "Decay", 5, 3000, 250, "ms"),
    P(3, "Sustain", 0, 100, 60, "%"),
    P(4, "Release", 10, 5000, 120, "ms"),
    P(5, "Level", -24, 6, 0, "dB"),
  ],
  "String Ensemble": [
    hz(0, "Cutoff", 100, 16000, 3200),
    P(1, "Detune", 0, 50, 8, "ct"),
    P(2, "Attack", 1, 3000, 180, "ms"),
    P(3, "Release", 10, 5000, 700, "ms"),
    P(4, "Level", -24, 6, 0, "dB"),
  ],
  "Tonewheel Organ": [
    P(0, "Attack", 1, 3000, 3, "ms"),
    P(1, "Release", 10, 5000, 60, "ms"),
    P(2, "Level", -24, 6, 0, "dB"),
  ],
};

/** Sound folder per mock plugin, as engine/src/control_plugins.rs files them. */
const MOCK_FOLDERS: Record<string, string> = {
  "Ondera Comp": "Dynamics",
  Gate: "Dynamics",
  Limiter: "Dynamics",
  Transient: "Dynamics",
  "De-Esser": "Dynamics",
  Pump: "Dynamics",
  "Channel EQ": "EQ & Filter",
  Filter: "EQ & Filter",
  "Auto Filter": "EQ & Filter",
  "Tape Sat": "Distortion",
  Overdrive: "Distortion",
  Bitcrusher: "Distortion",
  "Lo-Fi": "Distortion",
  Chorus: "Modulation",
  Phaser: "Modulation",
  Tremolo: "Modulation",
  Flanger: "Modulation",
  "Auto Pan": "Modulation",
  Space: "Space & Time",
  Echo: "Space & Time",
  "Pitch Shift": "Pitch",
  "Stereo Width": "Utility",
  Utility: "Utility",
  "Ondera Synth": "Synths",
  "Choir Pad": "Pads",
  "String Ensemble": "Pads",
  "Grand Piano": "Keys",
  "Tonewheel Organ": "Keys",
  "Drum Kit": "Drums",
  "Analog Bass": "Bass",
};
const INSTRUMENT_FOLDERS = ["Synths", "Keys", "Bass", "Drums", "Pads"];
const mockLibrary = {
  favorites: new Set<string>(["stock:Ondera Comp"]),
  folders: new Map<string, string>(),
  recent: ["stock:Space", "stock:Channel EQ"],
};
const mockFolder = (name: string) =>
  mockLibrary.folders.get(`stock:${name}`) ?? MOCK_FOLDERS[name] ?? "Utility";

if (panel.startsWith("plugin:")) {
  const name = panel.slice(7);
  const insert = strips.keys!.inserts[0]!;
  insert.name = name;
  insert.plugin = `stock:${name}`;
  ui.pluginWindows = [insert.id];
}
const pluginAt = (params: Params) => {
  const strip = strips[String(params.trackId)];
  return (
    (params.slot === undefined
      ? strip?.synth?.name
      : strip?.inserts[Number(params.slot)]?.name) ?? "Channel EQ"
  );
};
const values = new Map<string, number>();

function peaks() {
  const out: number[] = [];
  for (let i = 0; i < 4800; i++) {
    const env = 0.25 + 0.75 * Math.abs(Math.sin(i / 310)) ** 2;
    out.push(
      env * (0.35 + 0.65 * Math.abs(Math.sin(i * 1.7) * Math.cos(i / 9))),
    );
  }
  return out;
}

function command(method: string, params: Params): unknown {
  switch (method) {
    case "web.ready":
      return {
        session,
        ui,
        platform: "macos",
        version: "dev",
        catalog: {
          instruments: ["Ondera Synth", "Grand Piano", "Drum Kit", "Choir Pad"],
          effects: Object.keys(STOCK_PARAMETERS),
          loops: [{ name: "Night beat", instrument: "Drum Kit", bars: 4 }],
        },
      };
    case "plugin.list":
      return {
        nextOffset: null,
        plugins: Object.keys(STOCK_PARAMETERS).map((name) => {
          const instrument = INSTRUMENT_FOLDERS.includes(
            MOCK_FOLDERS[name] ?? "",
          );
          return {
            id: `stock:${name}`,
            name,
            vendor: "Ondera",
            instrument,
            effect: !instrument,
            format: "stock",
            folder: mockFolder(name),
            favorite: mockLibrary.favorites.has(`stock:${name}`),
          };
        }),
      };
    case "plugin.folders":
      return {
        folders: [
          ...INSTRUMENT_FOLDERS,
          "Dynamics",
          "EQ & Filter",
          "Distortion",
          "Modulation",
          "Space & Time",
          "Pitch",
          "Utility",
        ].map((name) => ({ name })),
        recent: mockLibrary.recent,
      };
    case "plugin.setFavorite":
      if (params.favorite) mockLibrary.favorites.add(String(params.pluginId));
      else mockLibrary.favorites.delete(String(params.pluginId));
      return {};
    case "plugin.setFolder":
      if (params.folder)
        mockLibrary.folders.set(String(params.pluginId), String(params.folder));
      else mockLibrary.folders.delete(String(params.pluginId));
      return {};
    case "web.peaks":
      return { peaks: peaks(), rate: 400 };
    case "settings.get":
      return settings;
    case "settings.set": {
      const [section, key] = String(params.path).split(".") as [
        keyof typeof settings,
        string,
      ];
      (settings[section] as Params)[key] = params.value;
      if (key === "appearance") ui.appearance = String(params.value);
      if (key === "mode") ui.mode = String(params.value);
      void emit("daw:ui", { ...ui });
      return {};
    }
    case "audio.devices":
      return { inputs: [], outputs: ["Built-in"], midiInputs: [] };
    case "strip.parameters": {
      const pluginName = pluginAt(params);
      return {
        pluginId: `stock:${pluginName}`,
        parameters: (STOCK_PARAMETERS[pluginName] ?? []).map((p) => ({
          ...p,
          value: values.get(`${pluginName}:${p.id}`) ?? p.value,
        })),
      };
    }
    case "strip.setParameter":
      values.set(
        `${pluginAt(params)}:${params.parameterId}`,
        Number(params.value),
      );
      return {};
    case "ui.closePluginWindow":
      ui.pluginWindows = ui.pluginWindows.filter((id) => id !== params.id);
      void emit("daw:ui", { ...ui });
      return {};
    case "preset.list":
      return { presets: [{ name: "Vocal glue", factory: true }] };
    case "track.setArmed":
    case "track.setMonitor": {
      const track = session.tracks.find((t) => t.id === params.trackId);
      if (track) {
        if (method === "track.setArmed") track.armed = Boolean(params.armed);
        else track.monitor = params.monitor as typeof track.monitor;
        void emit("daw:document", { ...session, tracks: [...session.tracks] });
      }
      return {};
    }
    case "agent.models":
      return [];
    case "agent.connection":
      return { provider: "claude", state: "configured", message: "" };
    case "ui.showPanel": {
      const name = String(params.panel);
      if (name in ui)
        (ui as Params)[name] = params.open ?? !(ui as Params)[name];
      void emit("daw:ui", { ...ui });
      return {};
    }
    default:
      return {};
  }
}

export function install(): void {
  mockWindows("main");
  mockIPC(
    (cmd, payload) => {
      const args = (payload ?? {}) as { method?: string; params?: Params };
      if (cmd === "daw_command")
        return command(args.method ?? "", args.params ?? {});
      return null;
    },
    { shouldMockEvents: true },
  );
  let phase = 0;
  setInterval(() => {
    phase += 0.13;
    const level = (o: number) => 0.35 + 0.3 * Math.abs(Math.sin(phase + o));
    void emit("daw:telemetry", {
      position: 13,
      playing: false,
      recording: false,
      peaks: [level(0), level(0.4), level(1), level(1.3)],
      trackPeaks: TRACKS.map((_, i) => level(i)),
      cpu: 0.23,
    });
  }, 120);
}
