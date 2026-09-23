import { describe, expect, it } from "vitest";
import { beatLineOffsets } from "./time";

describe("beatLineOffsets", () => {
  it("draws the meter's own beats, not always four", () => {
    expect(beatLineOffsets(120, { numerator: 4, denominator: 4 })).toEqual([
      30, 60, 90,
    ]);
    expect(beatLineOffsets(120, { numerator: 3, denominator: 4 })).toEqual([
      40, 80,
    ]);
    expect(beatLineOffsets(70, { numerator: 7, denominator: 8 })).toHaveLength(
      6,
    );
  });
  it("draws nothing when zoomed out or crowded, and survives nonsense", () => {
    expect(beatLineOffsets(20, { numerator: 4, denominator: 4 })).toEqual([]);
    expect(beatLineOffsets(30, { numerator: 16, denominator: 16 })).toEqual([]);
    expect(
      beatLineOffsets(Number.NaN, { numerator: 4, denominator: 4 }),
    ).toEqual([]);
    expect(beatLineOffsets(100, { numerator: 0, denominator: 4 })).toEqual([]);
  });
});
