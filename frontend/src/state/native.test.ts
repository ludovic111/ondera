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
        method: "web.trimClip",
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
  it("does not send browser search/selection to the audio engine", async () => {
    store.dispatch({ name: "view.setBrowserTab", params: { tab: "plugins" } });
    store.dispatch({
      name: "view.setBrowserSelection",
      params: { name: "Space" },
    });
    await flush();
    expect(edits()).toEqual([]);
    expect(store.getState().view.browserTab).toBe("plugins");
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
