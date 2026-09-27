import { describe, expect, it } from "vitest";
import {
  TempoMap,
  barAfterSeconds,
  barsSeconds,
  tempoMap,
  tempoSourceBar,
} from "./tempo";

const four = { numerator: 4, denominator: 4 };

describe("tempo map", () => {
  it("is the old arithmetic at one tempo", () => {
    const map = new TempoMap(120, [], 4);
    expect(map.isConstant).toBe(true);
    expect(map.seconds(8)).toBe(4);
    expect(map.beat(4)).toBe(8);
    expect(map.duration(2, 6)).toBe(2);
    expect(map.beatsFor(3, 1.5)).toBe(3);
  });

  it("changes tempo at a step and glides through a ramp, as the engine does", () => {
    // 120, then 60 from bar 1, ramping back to 120 by bar 3 (engine/tests/engine.rs).
    const map = new TempoMap(
      120,
      [
        { bar: 1, bpm: 60 },
        { bar: 3, bpm: 120, ramp: true },
      ],
      4,
    );
    expect(map.seconds(4)).toBe(2);
    expect(map.bpm(8)).toBeCloseTo(90, 9);
    const ramp = 8 * Math.LN2;
    expect(map.seconds(12)).toBeCloseTo(2 + ramp, 9);
    expect(map.seconds(16)).toBeCloseTo(4 + ramp, 9);
    for (const beat of [0, 3, 4, 5.5, 9, 12, 20])
      expect(map.beat(map.seconds(beat))).toBeCloseTo(beat, 9);
  });

  it("answers in bars for a session and reuses the map", () => {
    const state = {
      transport: { tempo: 120, timeSignature: four },
      tempoChanges: [{ bar: 2, bpm: 60 }],
    };
    expect(tempoMap(state)).toBe(tempoMap(state));
    expect(barsSeconds(state, 0, 2)).toBe(4);
    expect(barsSeconds(state, 1, 3)).toBe(2 + 4);
    expect(barAfterSeconds(state, 1, 6)).toBeCloseTo(3, 9);
    expect(tempoSourceBar(state, 7.9)).toBe(0);
    expect(tempoSourceBar(state, 8)).toBe(2);
  });
});
