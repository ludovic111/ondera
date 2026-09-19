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
  it("pastes a copied MIDI region at the playhead on its own track when the selected track is another kind", () => {
    const dispatch = vi.spyOn(store, "dispatch").mockImplementation(() => {});
    expect(actions.paste.enabled!(store.getState(), store)).toBe(false);
    runAction(store, "copy");
    expect(actions.paste.enabled!(store.getState(), store)).toBe(true);
    runAction(store, "paste");
    const { name, params } = dispatch.mock.calls[0][0];
    expect(name).toBe("clip.create");
    expect(params).toMatchObject({
      trackId: "t1",
      startBar: 0,
      lengthBars: 2,
      name: "Riff",
    });
    expect(params.notes).toEqual([
      { start: 0, length: 1, pitch: 60, velocity: 90 },
    ]);
  });
  it("cut copies, then removes", () => {
    const dispatch = vi.spyOn(store, "dispatch").mockImplementation(() => {});
    runAction(store, "cut");
    expect(dispatch.mock.calls[0][0]).toMatchObject({
      name: "clip.remove",
      params: { clipId: "c1" },
    });
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
