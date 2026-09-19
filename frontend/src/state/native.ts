import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { familyVar } from "../theme/families";
import type {
  BrowserItem,
  Command,
  Session,
  View,
  ChannelStrip,
  BrowserGroup,
  BrowserTab,
} from "@ondera/core";
import { library } from "../audio/library";

export type Params = Record<string, unknown>;
export interface UiState {
  dirty: boolean;
  path: string | null;
  history: { canUndo: boolean; canRedo: boolean };
  agentPanel: boolean;
  automation: boolean;
  settings: boolean;
  settingsSection: string;
  help: boolean;
  mixer?: boolean;
  export: boolean;
  recovery: boolean;
  tool: View["arrangeTool"];
  musicalTyping: boolean;
  pluginWindows: string[];
  status: string;
  error: string | null;
  busy: boolean;
  prompt: string | null;
  scale: number;
  /** Theme id; "graphite" and unknown values fall back to skeuo. */
  appearance?: string;
  mode?: "dark" | "light" | "auto";
  recoveryStatus: string;
  update: { available: string | null; installed: boolean; busy: boolean };
}
export interface Overlays {
  palette: boolean;
}
export interface AgentEntry {
  role: string;
  text: string;
  streaming?: boolean;
  tool?: { name: string; args: Params; ok: boolean; result: unknown };
}
export interface Change {
  sequence: number;
  title: string;
  detail: string;
  output: string;
  succeeded: boolean;
  running: boolean;
  mutated: boolean;
  applied: boolean;
}
export interface AgentData {
  status: {
    provider: string;
    model: string;
    reasoningEffort?: string;
    running: boolean;
    status: string;
    error: string | null;
    elapsedSeconds: number;
  };
  transcript: { entries: AgentEntry[] };
  changes: Change[];
}
export interface Plugin {
  id: string;
  name: string;
  vendor: string;
  instrument: boolean;
  effect: boolean;
  format: string;
  /** Sound folder: automatic, or where the user filed it. */
  folder: string;
  favorite: boolean;
}
export interface Catalog {
  instruments: string[];
  effects: string[];
  loops: { name: string; instrument: string; bars: number }[];
}
interface NativeStrip extends ChannelStrip {
  synth?: { id: string; name: string; plugin?: string };
  inserts: (ChannelStrip["inserts"][number] & {
    id?: string;
    plugin?: string;
  })[];
}
export interface DocumentData extends Omit<Session, "strips"> {
  snapshotSequence?: number;
  strips: Record<string, NativeStrip>;
  masterVolume: number;
  automation: AutomationLane[];
}
export interface AutomationLane {
  id: string;
  name?: string;
  target: Params;
  enabled: boolean;
  interpolation: string;
  points: { beat: number; value: number }[];
}
export const native = <T = unknown>(
  method: string,
  params: Params = {},
): Promise<T> => invoke<T>("daw_command", { method, params });
/** Commands that set an absolute value and are sent on every pointer move. */
const CONTINUOUS = new Set([
  "track.setVolume",
  "track.setPan",
  "master.setVolume",
  "strip.setSendLevel",
  "strip.setParameter",
  "transport.setTempo",
]);
const continuousKey = (name: string, params: Params): string | null =>
  CONTINUOUS.has(name)
    ? [
        name,
        params.trackId,
        params.slot,
        params.sendIndex ?? params.send,
        params.parameterId,
      ].join("|")
    : null;
/** Loops have no sound family yet; they cycle through the families for variety. */
const LOOP_SWATCHES = [
  "Drums",
  "Synths",
  "Pads",
  "Textures",
  "Keys",
  "Samplers",
  "Bass",
  "Distortion",
].map(familyVar);
const emptyGroups: Record<BrowserTab, BrowserGroup[]> = {
  instruments: [],
  plugins: [],
  loops: [],
  files: [],
};
const defaultView: View = {
  pixelsPerBar: 48,
  scrollBars: 0,
  agentPanelOpen: false,
  selectedTrackId: null,
  selectedClipId: null,
  editorClipId: null,
  selectedNoteId: null,
  editorMode: "pianoRoll",
  browserTab: "instruments",
  browserSelection: null,
  arrangeTool: "pointer",
  followPlayhead: true,
};
const empty: Session = {
  name: "Ondera",
  tracks: [],
  clips: [],
  sources: {},
  strips: {},
  view: defaultView,
  audio: { sampleRate: 48000, bitDepth: 24, bufferSize: 128 },
  transport: {
    playing: false,
    recording: false,
    positionBeats: 0,
    tempo: 120,
    timeSignature: { numerator: 4, denominator: 4 },
    key: "C min",
    cycle: false,
    cycleStartBar: 0,
    cycleEndBar: 4,
    metronome: false,
    snapDivision: 16,
  },
  meters: { masterL: 0, masterR: 0, channelL: 0, channelR: 0, cpu: 0 },
  browser: emptyGroups,
  agent: { status: "idle", transport: "", current: null, log: [], draft: "" },
};

/** Read-only mirror of Rust. Local state contains only view gestures and rendering telemetry. */
export class NativeStore {
  private state: Session = empty;
  private listeners = new Set<() => void>();
  private metadataListeners = new Set<() => void>();
  private queue: Promise<unknown> = Promise.resolve();
  /** Newest queued edit per continuous control; older ones are dropped unsent. */
  private newest = new Map<string, unknown>();
  private waiting = 0;
  private snapshotSequence = 0;
  private agentSequence = 0;
  private ids = new Map<string, string>();
  private unlisten: UnlistenFn[] = [];
  private localView: Partial<View> = {};
  document: DocumentData | null = null;
  catalog: Catalog = { instruments: [], effects: [], loops: [] };
  plugins: Plugin[] = [];
  platform = "";
  version = "";
  ui = {} as UiState;
  agent: AgentData = {
    status: {
      provider: "",
      model: "",
      running: false,
      status: "",
      error: null,
      elapsedSeconds: 0,
    },
    transcript: { entries: [] },
    changes: [],
  };
  /** Window-only overlays that the session and the host never need to know about. */
  overlays: Overlays = { palette: false };
  getOverlays = () => this.overlays;
  setOverlay = (name: keyof Overlays, open: boolean) => {
    if (this.overlays[name] === open) return;
    this.overlays = { ...this.overlays, [name]: open };
    this.notifyMeta();
  };
  /** Post-fader peak per track, in track order; read by the mixer's meters. */
  trackPeaks: number[] = [];
  private composer = { draft: "", sending: false, error: "" };
  getComposer = () => this.composer;
  setAgentDraft = (draft: string) => {
    this.composer = { ...this.composer, draft, error: "" };
    this.notifyMeta();
  };
  async sendAgent(): Promise<boolean> {
    const draft = this.composer.draft;
    if (!draft.trim() || this.composer.sending || this.agent.status.running)
      return false;
    this.composer = { ...this.composer, sending: true, error: "" };
    this.notifyMeta();
    const sequence = this.agentSequence;
    const task = this.queue.then(() =>
      native<AgentData["status"]>("agent.send", { prompt: draft.trim() }),
    );
    this.queue = task.catch(() => {});
    try {
      const status = await task;
      if (sequence === this.agentSequence)
        this.agent = { ...this.agent, status };
      this.composer = {
        ...this.composer,
        draft: this.composer.draft === draft ? "" : this.composer.draft,
      };
      return true;
    } catch (error) {
      this.composer = { ...this.composer, error: String(error) };
      return false;
    } finally {
      this.composer = { ...this.composer, sending: false };
      this.notifyMeta();
    }
  }
  getState = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  subscribeMeta = (listener: () => void) => {
    this.metadataListeners.add(listener);
    return () => {
      this.metadataListeners.delete(listener);
    };
  };
  getUi = () => this.ui;
  getAgent = () => this.agent;
  canUndo = () => this.ui.history?.canUndo ?? false;
  canRedo = () => this.ui.history?.canRedo ?? false;
  private notify = () => {
    for (const listener of this.listeners) listener();
  };
  private notifyMeta = () => {
    for (const listener of this.metadataListeners) listener();
  };
  dismissError = () => {
    this.ui = { ...this.ui, error: null };
    this.notifyMeta();
    this.fire("ui.dismissError");
  };
  reportError = (error: unknown) => {
    this.ui = { ...this.ui, error: String(error) };
    this.notifyMeta();
  };
  async connect() {
    this.unlisten = await Promise.all([
      listen<string[]>("daw:audioSources", (e) => {
        library.clear();
        for (const id of e.payload)
          void library.load(id).catch(this.reportError);
      }),
      listen<DocumentData>("daw:document", (e) => this.receive(e.payload)),
      listen<UiState>("daw:ui", (e) => this.receiveUi(e.payload)),
      listen<AgentData>("daw:agent", (e) => {
        this.agentSequence++;
        this.agent = e.payload;
        this.notifyMeta();
      }),
      listen<{
        position: number;
        playing: boolean;
        recording: boolean;
        countingIn?: boolean;
        inputPeak?: number;
        peaks: number[];
        trackPeaks?: number[];
        cpu: number;
      }>("daw:telemetry", (e) => {
        const t = e.payload;
        this.trackPeaks = t.trackPeaks ?? [];
        this.state = {
          ...this.state,
          transport: {
            ...this.state.transport,
            positionBeats: t.position,
            playing: t.playing,
            recording: t.recording,
            countingIn: t.countingIn ?? false,
          },
          meters: {
            masterL: t.peaks[0],
            masterR: t.peaks[1],
            channelL: t.peaks[2],
            channelR: t.peaks[3],
            cpu: t.cpu,
            input: t.inputPeak ?? 0,
          },
        };
        this.notify();
      }),
    ]);
    const initial = await native<{
      session: DocumentData;
      ui: UiState;
      catalog: Catalog;
      platform: string;
      version: string;
    }>("web.ready");
    this.catalog = initial.catalog;
    this.platform = initial.platform;
    this.version = initial.version;
    this.receive(initial.session);
    this.receiveUi(initial.ui);
    await this.refreshPlugins();
  }
  disconnect() {
    this.unlisten.forEach((fn) => fn());
  }
  private receiveUi(ui: UiState) {
    this.ui = ui;
    this.state = {
      ...this.state,
      view: {
        ...this.state.view,
        agentPanelOpen: ui.agentPanel,
        arrangeTool: ui.tool,
      },
    };
    this.notify();
    this.notifyMeta();
  }
  receive(doc: DocumentData) {
    if (doc.snapshotSequence !== undefined) {
      if (doc.snapshotSequence < this.snapshotSequence) return;
      this.snapshotSequence = doc.snapshotSequence;
    }
    if (doc.view.editorClipId !== this.state.view.editorClipId)
      delete this.localView.editorLowPitch;
    this.document = doc;
    const strips: Record<string, ChannelStrip> = {};
    for (const [id, strip] of Object.entries(doc.strips)) {
      strips[id] = {
        ...strip,
        instrument: strip.synth?.name ?? strip.instrument,
        input:
          doc.tracks.find((t) => t.id === id)?.kind === "midi"
            ? "Musical typing"
            : "Input",
        output: "Stereo Out",
        inserts: Array.from(
          { length: 8 },
          (_, i) =>
            strip.inserts[i] ?? {
              name: "Empty slot",
              state: "empty",
              meta: "",
            },
        ),
        sends: [0, 1].map((i) => ({
          name: i === 0 ? "A · Reverb" : "B · Delay",
          levelDb: strip.sends[i]?.levelDb ?? -Infinity,
        })),
      };
    }
    this.state = {
      ...this.state,
      ...doc,
      strips,
      browser: {
        ...this.state.browser,
        files: [
          {
            name: "Project audio",
            items: Object.values(doc.sources).map((s) => ({
              id: s.id,
              name: s.name,
              meta: `${s.durationSeconds.toFixed(1)} s`,
              color: familyVar("Samplers"),
            })),
          },
        ],
      },
      view: {
        ...defaultView,
        ...doc.view,
        ...this.localView,
        agentPanelOpen: this.ui.agentPanel ?? false,
      },
      transport: {
        ...doc.transport,
        playing: this.state.transport.playing,
        recording: this.state.transport.recording,
        positionBeats: this.state.transport.positionBeats,
      },
    };
    for (const id of Object.keys(doc.sources))
      void library.load(id).catch(() => {});
    this.notify();
    this.notifyMeta();
  }
  async refreshPlugins() {
    const plugins: Plugin[] = [];
    let offset = 0;
    while (true) {
      const page = await native<{
        plugins: Plugin[];
        nextOffset: number | null;
      }>("plugin.list", { offset, limit: 200 });
      plugins.push(...page.plugins);
      if (page.nextOffset == null) break;
      offset = page.nextOffset;
    }
    this.plugins = plugins;
    const library = await native<{
      folders: { name: string }[];
      recent: string[];
    }>("plugin.folders").catch(() => ({ folders: [], recent: [] }));
    const order = (library.folders ?? []).map((f) => f.name);
    const group = (
      items: { name: string; meta: string; color: string | null }[],
    ): BrowserGroup[] => [{ name: "Ondera", items }];
    const FORMAT: Record<string, string> = {
      native: "Rust",
      clap: "CLAP",
      vst3: "VST3",
      au: "AU",
    };
    const item = (p: Plugin): BrowserItem => ({
      id: p.id,
      name: p.name,
      meta:
        p.format === "stock"
          ? ""
          : `${p.vendor} · ${FORMAT[p.format] ?? p.format}`,
      color: familyVar(p.folder),
      favorite: p.favorite,
      folder: p.folder,
    });
    const groups = (kind: string): BrowserGroup[] => {
      const mine = plugins.filter((p) =>
        kind === "instrument" ? p.instrument : p.effect,
      );
      const byFolder = new Map<string, BrowserItem[]>();
      for (const p of mine) {
        if (!byFolder.has(p.folder)) byFolder.set(p.folder, []);
        byFolder.get(p.folder)!.push(item(p));
      }
      const rank = (name: string) =>
        order.includes(name) ? order.indexOf(name) : order.length;
      const folders = [...byFolder.entries()]
        .sort((a, b) => rank(a[0]) - rank(b[0]) || a[0].localeCompare(b[0]))
        .map(([name, items]) => ({
          name,
          kind: "folder" as const,
          color: familyVar(name),
          items,
        }));
      const favorites = mine.filter((p) => p.favorite).map(item);
      const recent = (library.recent ?? [])
        .map((id) => mine.find((p) => p.id === id))
        .filter((p): p is Plugin => p !== undefined)
        .slice(0, 6)
        .map(item);
      return [
        ...(favorites.length
          ? [
              {
                name: "Favourites",
                kind: "favorites" as const,
                items: favorites,
              },
            ]
          : []),
        ...(recent.length
          ? [{ name: "Recent", kind: "recent" as const, items: recent }]
          : []),
        ...folders,
      ];
    };
    const palette = LOOP_SWATCHES;
    this.state = {
      ...this.state,
      browser: {
        instruments: groups("instrument"),
        plugins: groups("effect"),
        loops: group(
          this.catalog.loops.map((l, i) => ({
            name: l.name,
            meta: `${l.bars} bars`,
            color: palette[i % palette.length] ?? null,
          })),
        ),
        files: this.state.browser.files,
      },
    };
    this.notify();
  }
  run<T = unknown>(method: string, params: Params = {}): Promise<T> {
    const task = this.queue.then(() => native<T>(method, params));
    this.queue = task.catch(this.reportError);
    return task;
  }
  /**
   * Fire and forget. Continuous absolute edits (a dial under the pointer) keep
   * only the newest queued value: the host never sees a backlog of stale ones.
   */
  fire(method: string, params: Params = {}) {
    const key = continuousKey(method, params);
    if (!key) {
      void this.run(method, params).catch(() => {});
      return;
    }
    const token = {};
    this.newest.set(key, token);
    const task = this.queue.then(() => {
      if (this.newest.get(key) !== token) return undefined;
      this.newest.delete(key);
      return native(method, params);
    });
    this.queue = task.catch(this.reportError);
  }
  dispatch = (command: Command): void => {
    const key = continuousKey(command.name, command.params);
    if (key) this.newest.set(key, command);
    this.waiting++;
    const task = this.queue.then(() => {
      this.waiting--;
      if (key) {
        if (this.newest.get(key) !== command) return undefined;
        this.newest.delete(key);
      }
      return this.translate(command, key !== null);
    });
    this.queue = task.catch(this.reportError);
  };
  setEditorPitch(low: number) {
    this.local({ editorLowPitch: low });
  }
  private local(patch: Partial<View>) {
    this.localView = { ...this.localView, ...patch };
    this.state = { ...this.state, view: { ...this.state.view, ...patch } };
    this.notify();
  }
  private async translate({ name, params }: Command, continuous = false) {
    const p = { ...params };
    for (const field of ["trackId", "clipId", "noteId"])
      if (typeof p[field] === "string")
        p[field] = this.ids.get(p[field] as string) ?? p[field];
    const s = this.state;
    let method = name;
    switch (name) {
      case "view.setBrowserTab":
        this.local({ browserTab: p.tab as View["browserTab"] });
        return;
      case "view.setBrowserSelection":
        this.local({ browserSelection: (p.name as string) ?? null });
        return;
      case "agent.setDraft":
        this.state = {
          ...s,
          agent: { ...s.agent, draft: String(p.text ?? "") },
        };
        this.notify();
        return;
      case "transport.togglePlay":
        method = s.transport.playing ? "transport.stop" : "transport.play";
        break;
      case "transport.setPosition":
        method = "transport.locate";
        break;
      case "transport.nudge":
        method = "transport.locate";
        p.beats = Math.max(
          0,
          s.transport.positionBeats +
            (Number(p.bars) * s.transport.timeSignature.numerator * 4) /
              s.transport.timeSignature.denominator,
        );
        delete p.bars;
        break;
      case "transport.setRecording":
        method = "transport.punch";
        p.enabled = p.recording;
        delete p.recording;
        break;
      case "transport.setCycleRange":
        method = "transport.setCycle";
        p.enabled = true;
        break;
      case "track.add": {
        const clientId = p.trackId;
        delete p.trackId;
        const index = p.index;
        delete p.index;
        const track = await native<{ id: string }>("track.add", p);
        if (typeof clientId === "string") this.ids.set(clientId, track.id);
        if (typeof index === "number")
          await native("track.move", { trackId: track.id, index });
        this.receive(await native<DocumentData>("web.document"));
        return;
      }
      case "clip.create": {
        const clientId = p.clipId;
        delete p.clipId;
        const clip = await native<{ id: string }>("clip.create", p);
        if (typeof clientId === "string") this.ids.set(clientId, clip.id);
        this.receive(await native<DocumentData>("web.document"));
        return;
      }
      case "track.duplicate": {
        const track = await native<{ id: string }>("track.duplicate", p);
        await native("track.select", { trackId: track.id });
        this.receive(await native<DocumentData>("web.document"));
        return;
      }
      case "clip.duplicate":
      case "clip.split":
        delete p.newClipId;
        if (name === "clip.split") {
          p.bar = p.atBar;
          delete p.atBar;
        }
        break;
      case "clip.resize":
        if ("startBar" in p) method = "clip.trim";
        break;
      case "clip.clearSelection":
        method = "clip.deselect";
        break;
      case "note.add": {
        const clientId = p.noteId;
        delete p.noteId;
        const note = await native<{ id: string }>("note.add", p);
        if (typeof clientId === "string") this.ids.set(clientId, note.id);
        this.receive(await native<DocumentData>("web.document"));
        return;
      }
      case "note.select":
        method = "clip.select";
        p.clipId = s.view.editorClipId;
        if (p.noteId == null) delete p.noteId;
        break;
      case "view.setAgentPanelOpen":
        method = "ui.showPanel";
        p.panel = "agent";
        p.visible = p.open;
        delete p.open;
        break;
      case "view.setArrangeTool":
        method = "ui.setTool";
        if (p.tool === "grid") p.tool = "pointer";
        break;
      case "view.setEditorClip":
        method = "view.set";
        p.editorClipId = p.clipId;
        delete p.clipId;
        break;
      case "view.setEditorMode":
        method = "view.set";
        p.editorMode = p.mode;
        delete p.mode;
        break;
      case "view.setFollowPlayhead":
        method = "view.set";
        p.followPlayhead = p.enabled;
        delete p.enabled;
        break;
      case "view.scrollTo":
      case "view.scrollBy":
        method = "view.set";
        p.scrollBar = Math.max(
          0,
          name === "view.scrollTo"
            ? Number(p.bar)
            : s.view.scrollBars + Number(p.bars),
        );
        delete p.bar;
        delete p.bars;
        break;
      case "view.setZoom":
      case "view.zoomBy": {
        const zoom = Math.max(
          12,
          Math.min(
            480,
            name === "view.zoomBy"
              ? s.view.pixelsPerBar * Number(p.factor)
              : Number(p.pixelsPerBar),
          ),
        );
        method = "view.set";
        p.pixelsPerBar = zoom;
        delete p.factor;
        if (typeof p.anchorBar === "number" && typeof p.anchorPx === "number")
          p.scrollBar = Math.max(0, p.anchorBar - p.anchorPx / zoom);
        delete p.anchorBar;
        delete p.anchorPx;
        break;
      }
      case "strip.setInsert":
        method = "strip.setInsert";
        p.slot = p.slotIndex;
        p.effect = p.name;
        delete p.slotIndex;
        delete p.name;
        break;
      case "strip.setInsertState": {
        method = p.state === "empty" ? "strip.setInsert" : "strip.setBypass";
        p.slot = p.slotIndex;
        if (p.state !== "empty") p.bypassed = p.state === "bypassed";
        delete p.slotIndex;
        delete p.state;
        break;
      }
      case "strip.setSendLevel":
        p.send = p.sendIndex;
        delete p.sendIndex;
        if (!Number.isFinite(p.levelDb)) delete p.levelDb;
        break;
      case "agent.stopCurrent":
        method = "agent.stop";
        break;
      case "agent.submit":
        method = "agent.send";
        p.prompt = s.agent.draft;
        delete p.id;
        break;
      default:
        break;
    }
    await native(method, p);
    // Mid-drag the next value is already queued; the snapshot after the last one is enough.
    if (continuous && this.waiting > 0) return;
    if (!name.startsWith("agent."))
      this.receive(await native<DocumentData>("web.document"));
  }
}
