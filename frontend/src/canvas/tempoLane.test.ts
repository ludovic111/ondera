import { describe, expect, it } from "vitest";
import type { Session } from "@ryolune/core";
import {
  bpmAtY,
  bpmLabel,
  tempoAt,
  tempoPoints,
  tempoRange,
  yOfBpm,
} from "./tempoLane";

const H = 56;
const song = (tempoChanges: Session["tempoChanges"]) =>
  ({
    transport: {
      tempo: 120,
      timeSignature: { numerator: 4, denominator: 4 },
      positionBeats: 0,
    },
    view: { pixelsPerBar: 40, scrollBars: 0 },
    tracks: [],
    tempoChanges,
  }) as unknown as Session;

describe("tempo track", () => {
  const state = song([
    { bar: 4, bpm: 90 },
    { bar: 8, bpm: 140, ramp: true },
  ]);

  it("lists the starting tempo as bar 0, then the changes, a drag applied", () => {
    expect(tempoPoints(state).map((p) => p.bar)).toEqual([0, 4, 8]);
    const dragged = tempoPoints(state, { from: 4, bar: 10, bpm: 100 });
    expect(dragged.map((p) => [p.bar, p.bpm])).toEqual([
      [0, 120],
      [8, 140],
      [10, 100],
    ]);
  });

  it("frames every tempo with room, in tens, and maps BPM to height and back", () => {
    const range = tempoRange(tempoPoints(state));
    expect(range).toEqual([80, 150]);
    expect(tempoRange([{ bar: 0, bpm: 25 }])).toEqual([20, 40]);
    for (const bpm of [80, 97.5, 150])
      expect(bpmAtY(yOfBpm(bpm, range, H), range, H)).toBeCloseTo(bpm, 9);
    expect(yOfBpm(150, range, H)).toBeLessThan(yOfBpm(80, range, H));
  });

  it("finds the point under the pointer, or the starting tempo's line", () => {
    const range = tempoRange(tempoPoints(state));
    const at = (bar: number, bpm: number) =>
      tempoAt(state, bar * 40, yOfBpm(bpm, range, H), H);
    expect(at(4, 90)).toEqual({ bar: 4 });
    expect(at(8, 140)).toEqual({ bar: 8 });
    expect(at(1, 120)).toEqual({ bar: 0 });
    // After the first change the starting tempo is no longer there to grab.
    expect(at(6, 120)).toBeNull();
    expect(at(2, 100)).toBeNull();
  });

  it("labels whole tempos without decimals", () => {
    expect(bpmLabel(120)).toBe("120");
    expect(bpmLabel(117.94)).toBe("117.9");
    expect(bpmLabel(89.999)).toBe("90");
  });
});
