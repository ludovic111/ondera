import { beforeEach, expect, it, vi } from "vitest";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "./native";
import { buildMenu, type MenuItem } from "./menus";

let store: NativeStore;
let commands: { method: string; params: Record<string, unknown> }[];
let picks: Record<string, unknown>[];
let picked: string | null;
beforeEach(() => {
  mock.invoke.mockReset();
  store = new NativeStore();
  const state = store.getState();
  const doc: DocumentData = {
    ...state,
    name: "Night Drive.ondera",
    strips: {},
    masterVolume: 0.75,
    automation: [],
  };
  commands = [];
  picks = [];
  picked = "/Users/me/Song.mid";
  mock.invoke.mockImplementation(async (command, args) => {
    if (command === "daw_pick") {
      picks.push(args);
      return picked;
    }
    if (args.method === "web.document") return doc;
    commands.push(args);
    return {};
  });
  store.receive(doc);
  // The playhead arrives with telemetry, not with the document.
  const live = store as unknown as { state: DocumentData };
  live.state = {
    ...live.state,
    transport: { ...live.state.transport, positionBeats: 9 },
  };
});
const item = (label: string) =>
  buildMenu("File", store).find(
    (e): e is MenuItem => "label" in e && e.label === label,
  )!;
const settle = () => new Promise((r) => setTimeout(r, 0));

// Both items used to open the audio export dialog: the MIDI modes of the old egui dialog
// are never drawn in the Tauri window.
it("File > Import MIDI… picks a file and imports it at the playhead's bar", async () => {
  item("Import MIDI…").onSelect!();
  await settle();
  await store.run("barrier");
  expect(picks).toEqual([{ kind: "midi" }]);
  expect(commands).toContainEqual({
    method: "session.importMidi",
    params: { path: "/Users/me/Song.mid", startBar: 2 },
  });
  expect(commands.some((c) => c.params.action === "export")).toBe(false);
});

it("File > Export MIDI… saves a .mid named after the session", async () => {
  picked = "/Users/me/Night Drive";
  item("Export MIDI…").onSelect!();
  await settle();
  await store.run("barrier");
  expect(picks).toEqual([{ kind: "saveMidi", name: "Night Drive.mid" }]);
  expect(commands).toContainEqual({
    method: "session.exportMidi",
    params: { path: "/Users/me/Night Drive.mid" },
  });
});

it("a cancelled chooser does nothing", async () => {
  picked = null;
  item("Import MIDI…").onSelect!();
  await settle();
  await store.run("barrier");
  expect(commands.filter((c) => c.method !== "barrier")).toEqual([]);
});
