import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore, type DocumentData } from "./native";
import { actions, runAction } from "./actions";
import {
  matchScore,
  paletteEntries,
} from "../components/palette/CommandPalette";
import { formatShortcut } from "./shortcuts";

let store: NativeStore;
const doc = (): DocumentData =>
  ({
    ...store.getState(),
    masterVolume: 0.75,
    automation: [],
    strips: {},
    tracks: [
      {
        id: "t1",
        name: "Keys",
        kind: "midi",
        color: "#fff",
        volume: 0.75,
        pan: 0,
        mute: false,
        solo: false,
        armed: false,
        agentActive: false,
      },
      {
        id: "t2",
        name: "Vox",
        kind: "audio",
        color: "#fff",
        volume: 0.75,
        pan: 0,
        mute: false,
        solo: false,
        armed: false,
        agentActive: false,
      },
    ],
    clips: [
      {
        id: "c1",
        trackId: "t1",
        name: "Riff",
        startBar: 0,
        lengthBars: 2,
        agent: false,
        data: {
          kind: "midi",
          notes: [{ id: "n1", start: 0, length: 1, pitch: 60, velocity: 90 }],
        },
      },
    ],
    view: {
      ...store.getState().view,
      selectedClipId: "c1",
      selectedTrackId: "t2",
    },
  }) as DocumentData;

beforeEach(() => {
  store = new NativeStore();
  store.receive(doc());
});

describe("clipboard", () => {
  // The host holds the clipboard and chooses where a paste lands (engine test
  // `the_clipboard_lives_in_the_host_so_any_client_can_paste`), so the window only says what
  // the person did.
  it("copies and cuts the selected region, and pastes at the playhead bar", () => {
    const fire = vi.spyOn(store, "fire").mockImplementation(() => {});
    runAction(store, "copy");
    runAction(store, "cut");
    runAction(store, "paste");
    expect(fire.mock.calls).toEqual([
      ["clip.copy", { clipId: "c1" }],
      ["clip.cut", { clipId: "c1" }],
      ["clip.paste", { bar: 0 }],
    ]);
  });
  it("fits the session through the host, which knows the lane width", () => {
    const fire = vi.spyOn(store, "fire").mockImplementation(() => {});
    runAction(store, "zoomToFit");
    expect(fire).toHaveBeenCalledWith("view.fit");
  });
});

describe("command palette", () => {
  it("ranks prefixes over substrings and rejects non-matches", () => {
    expect(matchScore("Mixer View", "mix")).toBeGreaterThan(
      matchScore("Show Mixer", "mix"),
    );
    expect(matchScore("Duplicate Track", "dptk")).toBe(1);
    expect(matchScore("Undo", "zzz")).toBe(0);
  });
  it("reaches every action, including the transport ones no menu lists", () => {
    const entries = paletteEntries(store);
    const keys = new Set(entries.map((e) => e.shortcut));
    for (const def of Object.values(actions))
      if (def.shortcut)
        expect(keys, def.id).toContain(formatShortcut(def.shortcut));
    expect(new Set(entries.map((e) => e.label)).size).toBe(entries.length);
  });
});
