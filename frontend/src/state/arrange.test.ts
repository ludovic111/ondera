import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore, type DocumentData } from "./native";
import { actions, runAction } from "./actions";
import { formatShortcut } from "./shortcuts";
import {
  clampFades,
  clipEnvelope,
  envelopeAt,
  fadeGain,
  FADE_CURVES,
  type Command,
} from "@ondera/core";
import { fadeHandleAt, markersWith, nameStart } from "../canvas/timeline";
import { markerAt } from "../canvas/markers";
import { describeTool } from "../components/agent/toolSteps";
import { size } from "../theme/tokens";

let store: NativeStore;
const doc = (positionBeats = 13): DocumentData =>
  ({
    ...store.getState(),
    masterVolume: 0.75,
    automation: [],
    strips: {},
    tracks: [
      {
        id: "vox",
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
        id: "a1",
        trackId: "vox",
        name: "Take",
        startBar: 2,
        lengthBars: 4,
        agent: false,
        data: {
          kind: "audio",
          sourceId: "s",
          offsetSeconds: 0,
          fadeInSeconds: 1,
          gainDb: -6,
        },
      },
    ],
    markers: [
      { id: "m1", bar: 0, name: "Intro" },
      { id: "m2", bar: 4, name: "Verse 1" },
      { id: "m3", bar: 8, name: "Chorus" },
    ],
    transport: {
      ...store.getState().transport,
      tempo: 120,
      positionBeats,
      snapDivision: 16,
    },
    view: { ...store.getState().view, pixelsPerBar: 40, scrollBars: 0 },
  }) as DocumentData;

beforeEach(() => {
  store = new NativeStore();
  store.receive(doc());
});

const dispatched = () => {
  const calls: Command[] = [];
  vi.spyOn(store, "dispatch").mockImplementation((c) => void calls.push(c));
  return calls;
};

describe("markers", () => {
  it("adds a marker at the playhead on the grid, unless one is already there", () => {
    const calls = dispatched();
    // The playhead position comes from telemetry, not the document.
    store.getState().transport.positionBeats = 21;
    runAction(store, "addMarker");
    expect(calls).toEqual([{ name: "marker.add", params: { bar: 5.25 } }]);
    store.getState().transport.positionBeats = 16;
    expect(actions.addMarker.enabled!(store.getState(), store)).toBe(false);
  });
  it("goes to the next and previous markers and cycles the section", () => {
    const calls = dispatched();
    store.getState().transport.positionBeats = 20; // bar 5, inside Verse 1
    runAction(store, "nextMarker");
    runAction(store, "previousMarker");
    runAction(store, "cycleSection");
    expect(calls.map((c) => c.name)).toEqual([
      "marker.next",
      "marker.previous",
      "marker.cycleSection",
    ]);
    store.getState().transport.positionBeats = 40; // past the last marker
    expect(actions.nextMarker.enabled!(store.getState(), store)).toBe(false);
  });
  it("keeps markers absent from old documents empty rather than stale", () => {
    const { markers: _drop, ...old } = doc();
    store.receive(old as DocumentData);
    expect(store.getState().markers).toEqual([]);
  });
  it("hits a flag in the ruler's lower half and applies a drag in progress", () => {
    const s = store.getState();
    const y = size.markerTop + 4;
    // Verse 1 sits at 4 bars × 40 px.
    expect(markerAt(s, 162, y)?.id).toBe("m2");
    expect(markerAt(s, 162, 2)).toBeNull();
    expect(markerAt(s, 30, y)?.id).toBe("m1");
    expect(markerAt(s, 100, y)).toBeNull();
    const moved = markersWith(s, { id: "m1", bar: 6 });
    expect(moved.map((m) => m.id)).toEqual(["m2", "m1", "m3"]);
    expect(markerAt(s, 242, y, { id: "m1", bar: 6 })?.id).toBe("m1");
  });
});

describe("audio clip fades and gain", () => {
  it("mirrors the engine's curves and clamping", () => {
    for (const curve of FADE_CURVES) {
      expect(fadeGain(curve, 0)).toBe(0);
      expect(fadeGain(curve, 1)).toBeCloseTo(1, 12);
    }
    expect(fadeGain("linear", 0.5)).toBe(0.5);
    expect(fadeGain("equalPower", 0.5)).toBeCloseTo(Math.SQRT1_2, 12);
    expect(fadeGain("exponential", 0.5)).toBeLessThan(0.2);
    expect(clampFades(3, 3, 4)).toEqual([2, 2]);
    expect(clampFades(1, 9, 4)).toEqual([0.8, 3.2]);
    expect(clampFades(-1, 1, 4)).toEqual([0, 1]);
  });
  it("draws the waveform with the gain and the fades it plays with", () => {
    const clip = store.getState().clips[0]!;
    if (clip.data.kind !== "audio") throw new Error("audio");
    const env = clipEnvelope(clip.data);
    expect(env).toEqual({
      fadeIn: 1,
      fadeOut: 0,
      curve: "equalPower",
      gainDb: -6,
    });
    const half = Math.pow(10, -6 / 20);
    expect(envelopeAt(env, 0.5, 7)).toBeCloseTo(half * Math.SQRT1_2, 9);
    expect(envelopeAt(env, 3, 5)).toBeCloseTo(half, 9);
  });
  it("starts a clip's name after a fade-in handle that would cover it", () => {
    const s = store.getState();
    const clip = s.clips[0]!;
    if (clip.data.kind !== "audio") throw new Error("audio");
    const ctx = { measureText: (t: string) => ({ width: t.length * 6 }) };
    const measure = ctx as unknown as CanvasRenderingContext2D;
    const env = clipEnvelope(clip.data);
    // 20 px a second: a 1 s fade puts the handle 20 px in, on a 60 px name.
    const after = nameStart(measure, "LV_v2_comp", env, 80, 160, false, s);
    expect(after).toBeGreaterThanOrEqual(80 + 20 + size.fadeHandle / 2);
    // A name that ends before the handle stays where it was.
    expect(nameStart(measure, "V", env, 80, 160, false, s)).toBe(86);
    // No fades and not selected: no handles, no shift.
    const none = { ...env, fadeIn: 0, fadeOut: 0 };
    expect(nameStart(measure, "LV_v2_comp", none, 80, 160, false, s)).toBe(86);
  });
  it("finds the fade handles in the clip's title strip", () => {
    const s = store.getState();
    const clip = s.clips[0]!;
    // Two seconds a bar at 120 BPM in 4/4: 20 px a second; the clip spans 80-240 px.
    const top = size.clipInset + 4;
    expect(fadeHandleAt(s, clip, 100, top)).toBe("in");
    expect(fadeHandleAt(s, clip, 238, top)).toBe("out");
    expect(fadeHandleAt(s, clip, 160, top)).toBeNull();
    expect(
      fadeHandleAt(s, clip, 100, size.clipInset + size.clipTitle + 10),
    ).toBe(null);
  });
});

describe("the action table", () => {
  it("never binds two actions to the same keys", () => {
    const seen = new Map<string, string>();
    for (const def of Object.values(actions)) {
      if (!def.shortcut) continue;
      const key = formatShortcut(def.shortcut);
      expect(seen.get(key), `${def.id} and ${seen.get(key)}`).toBeUndefined();
      seen.set(key, def.id);
    }
  });
  it("describes the new agent tools in plain words", () => {
    const say = (name: string, args = {}) =>
      describeTool({ name, args, ok: true, result: {} });
    expect(say("marker_add", { name: "Chorus" })).toBe(
      "Marked a section “Chorus”",
    );
    expect(say("clip_setGain", { gainDb: -3 })).toBe(
      "Set a region's gain to -3 dB",
    );
    expect(say("marker_list")).toBe("Looked at the song sections");
  });
});
