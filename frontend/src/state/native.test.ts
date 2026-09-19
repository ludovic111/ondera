import { beforeEach, describe, expect, it, vi } from "vitest";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "./native";
import { library } from "../audio/library";
import { buildMenu, MENU_TITLES } from "./menus";

let store: NativeStore;
let doc: DocumentData;
let calls: { method: string; params: Record<string, unknown> }[];
beforeEach(() => {
  library.clear();
  mock.invoke.mockReset();
  store = new NativeStore();
  doc = { ...store.getState(), strips: {}, masterVolume: 0.75, automation: [] };
  calls = [];
  mock.invoke.mockImplementation(async (_command, args) => {
    calls.push(args);
    if (args.method === "web.document") return doc;
    if (args.method === "track.add") return { id: "rust-track" };
    if (args.method === "clip.create") return { id: "rust-clip" };
    if (args.method === "note.add") return { id: "rust-note" };
    return {};
  });
  store.receive(doc);
});
const flush = () => store.run("barrier");
const edits = () =>
  calls.filter((c) => !["barrier", "web.document"].includes(c.method));
describe("Rust document ownership", () => {
  it("sends only the newest value of a fader drag and one snapshot", async () => {
    for (const volume of [0.1, 0.2, 0.3, 0.4])
      store.dispatch({
        name: "track.setVolume",
        params: { trackId: "t", volume },
      });
    store.dispatch({
      name: "track.setVolume",
      params: { trackId: "u", volume: 0.9 },
    });
    await flush();
    expect(edits().map((c) => c.params)).toEqual([
      { trackId: "t", volume: 0.4 },
      { trackId: "u", volume: 0.9 },
    ]);
    expect(calls.filter((c) => c.method === "web.document")).toHaveLength(1);
  });
  it("ignores older command snapshots arriving after a newer event", () => {
    store.receive({ ...doc, snapshotSequence: 20, name: "New session" });
    store.receive({ ...doc, snapshotSequence: 19, name: "Old session" });
    expect(store.getState().name).toBe("New session");
  });
  it("uses IDs returned by Rust for queued track, region and note operations", async () => {
    store.dispatch({
      name: "track.add",
      params: { trackId: "client-track", kind: "midi" },
    });
    store.dispatch({
      name: "clip.create",
      params: {
        trackId: "client-track",
        clipId: "client-clip",
        startBar: 0,
        lengthBars: 4,
      },
    });
    store.dispatch({
      name: "note.add",
      params: {
        clipId: "client-clip",
        noteId: "client-note",
        start: 0,
        length: 1,
        pitch: 60,
      },
    });
    store.dispatch({
      name: "note.update",
      params: { clipId: "client-clip", noteId: "client-note", pitch: 64 },
    });
    await flush();
    expect(edits()).toEqual([
      { method: "track.add", params: { kind: "midi" } },
      {
        method: "clip.create",
        params: { trackId: "rust-track", startBar: 0, lengthBars: 4 },
      },
      {
        method: "note.add",
        params: { clipId: "rust-clip", start: 0, length: 1, pitch: 60 },
      },
      {
        method: "note.update",
        params: { clipId: "rust-clip", noteId: "rust-note", pitch: 64 },
      },
    ]);
  });
  it("keeps a rejected edit out of the document and continues subsequent commands", async () => {
    mock.invoke.mockImplementationOnce(async () => {
      throw new Error("Plugin unavailable");
    });
    store.dispatch({ name: "track.add", params: { kind: "midi" } });
    await flush();
    expect(store.getState().tracks).toEqual([]);
    expect(store.ui.error).toContain("Plugin unavailable");
  });
  it("sends left-edge trimming as one native operation", async () => {
    store.dispatch({
      name: "clip.resize",
      params: { clipId: "audio", startBar: 2, lengthBars: 3 },
    });
    await flush();
    expect(edits()).toEqual([
      {
        method: "clip.trim",
        params: { clipId: "audio", startBar: 2, lengthBars: 3 },
      },
    ]);
  });
  it("sends absent send level for silence, never non-finite JSON", async () => {
    store.dispatch({
      name: "strip.setSendLevel",
      params: { trackId: "t", sendIndex: 1, levelDb: -Infinity },
    });
    await flush();
    expect(edits()).toEqual([
      { method: "strip.setSendLevel", params: { trackId: "t", send: 1 } },
    ]);
  });
  it("keeps gesture boundaries ordered around changes", async () => {
    store.fire("web.gesture", { active: true });
    store.dispatch({
      name: "track.setVolume",
      params: { trackId: "t", volume: 0.6 },
    });
    store.fire("web.gesture", { active: false });
    await flush();
    expect(edits().map((c) => c.method)).toEqual([
      "web.gesture",
      "track.setVolume",
      "web.gesture",
    ]);
  });
  it("keeps the browser tab and selection in the host's view, shown before the host answers", async () => {
    store.dispatch({ name: "view.setBrowserTab", params: { tab: "plugins" } });
    store.dispatch({
      name: "view.setBrowserSelection",
      params: { name: "Space" },
    });
    await flush();
    // A view change, not a document edit: two view.set calls and nothing else.
    expect(edits()).toEqual([
      { method: "view.set", params: { browserTab: "plugins" } },
      { method: "view.set", params: { browserSelection: "Space" } },
    ]);
    // The snapshot fetched meanwhile still says "instruments"; the window does not flicker back.
    expect(store.getState().view.browserTab).toBe("plugins");
    // Once the host's view changes under it (a CLI or an agent), the window follows.
    store.receive({ ...doc, view: { ...doc.view, browserTab: "loops" } });
    expect(store.getState().view.browserTab).toBe("loops");
  });
  it("opens the command palette when the host says so, and tells the host when the person does", async () => {
    const ui = { ...store.ui, palette: false };
    store.receiveUi(ui);
    expect(store.getOverlays().palette).toBe(false);
    // ondera-cli ui.showPanel panel=palette
    store.receiveUi({ ...ui, palette: true });
    expect(store.getOverlays().palette).toBe(true);
    // An unrelated update that still carries the old value does not close what was just opened.
    store.receiveUi({ ...ui, palette: true, status: "Saved" });
    store.setOverlay("palette", false);
    store.receiveUi({ ...ui, palette: true, status: "Ready" });
    expect(store.getOverlays().palette).toBe(false);
    await flush();
    expect(edits()).toEqual([
      { method: "ui.showPanel", params: { panel: "palette", visible: false } },
    ]);
  });
  it("provides actions for every enabled menu entry", () => {
    for (const title of MENU_TITLES)
      for (const entry of buildMenu(title, store)) {
        if (!entry.separator && !entry.disabled)
          expect(entry.onSelect, entry.label).toBeTypeOf("function");
      }
  });
});
describe("native waveform cache", () => {
  it("uses real peaks and their source-specific sampling rate", async () => {
    mock.invoke.mockResolvedValue({ peaks: [0, 0.5, 1], rate: 44100 / 110 });
    await library.load("source");
    expect([...library.peaksFor("source")!]).toEqual([0, 0.5, 1]);
    expect(library.rateFor("source")).toBe(44100 / 110);
  });
  it("ignores a response from a session that has been replaced", async () => {
    let finish!: (v: unknown) => void;
    mock.invoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const old = library.load("source");
    library.clear();
    finish({ peaks: [1], rate: 400 });
    await old;
    expect(library.peaksFor("source")).toBeUndefined();
  });
});

describe("agent composer", () => {
  it("retains the draft and explains a rejected send without opening a global error", async () => {
    store.setAgentDraft("Write a warm bass line");
    mock.invoke.mockRejectedValueOnce("Bridge unavailable");
    expect(await store.sendAgent()).toBe(false);
    expect(store.getComposer()).toMatchObject({
      draft: "Write a warm bass line",
      sending: false,
      error: "Bridge unavailable",
    });
    expect(store.ui.error).toBeUndefined();
  });
  it("prevents duplicate sends while the first request is pending", async () => {
    let finish!: (value: unknown) => void;
    mock.invoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    store.setAgentDraft("Add drums");
    const first = store.sendAgent();
    expect(await store.sendAgent()).toBe(false);
    await Promise.resolve();
    finish({ ...store.agent.status, running: true });
    expect(await first).toBe(true);
    expect(mock.invoke).toHaveBeenCalledTimes(1);
    expect(store.getComposer().draft).toBe("");
    expect(await store.sendAgent()).toBe(false);
  });
  it("keeps a new draft typed while an accepted request was in flight", async () => {
    let finish!: (value: unknown) => void;
    mock.invoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    store.setAgentDraft("Add drums");
    const first = store.sendAgent();
    await Promise.resolve();
    store.setAgentDraft("Now add bass");
    finish({ ...store.agent.status, running: true });
    await first;
    expect(store.getComposer().draft).toBe("Now add bass");
  });
});

describe("agent status ordering", () => {
  it("does not replace a newer completion event with an older send acknowledgement", async () => {
    let receive!: (event: { payload: typeof store.agent }) => void;
    mock.listen.mockImplementation(async (name, callback) => {
      if (name === "daw:agent") receive = callback;
      return () => {};
    });
    mock.invoke.mockImplementation(async (_command, args) => {
      if (args.method === "web.ready")
        return {
          session: doc,
          ui: { agentPanel: true },
          catalog: { instruments: [], effects: [], loops: [] },
          platform: "macos",
          version: "test",
        };
      if (args.method === "plugin.list")
        return { plugins: [], nextOffset: null };
      return {};
    });
    await store.connect();
    let finish!: (value: unknown) => void;
    mock.invoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    store.setAgentDraft("Inspect this project");
    const sent = store.sendAgent();
    await Promise.resolve();
    receive({
      payload: {
        ...store.agent,
        status: { ...store.agent.status, status: "Done", running: false },
      },
    });
    finish({ ...store.agent.status, status: "Starting…", running: true });
    await sent;
    expect(store.agent.status.status).toBe("Done");
    expect(store.agent.status.running).toBe(false);
    store.disconnect();
  });
});
