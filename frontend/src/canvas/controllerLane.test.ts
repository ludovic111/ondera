import { describe, expect, it } from "vitest";
import type { Clip, Controller } from "@ondera/core";
import {
  hitPoint,
  laneOf,
  laneParams,
  lanePoints,
  lanesInClip,
  laneTitle,
  parseCcNumber,
  sameLane,
  strokePoints,
  valueAtY,
  valueLabel,
  yOfValue,
  type Lane,
} from "./controllerLane";

const MOD: Lane = { kind: "cc", number: 1 };
const BEND: Lane = { kind: "bend" };

const point = (
  id: string,
  kind: Controller["kind"],
  time: number,
  value: number,
  number?: number,
): Controller => ({
  id,
  kind,
  time,
  value,
  ...(number === undefined ? {} : { number }),
});
const clip = (controllers?: Controller[]): Clip => ({
  id: "c",
  trackId: "t",
  name: "Clip",
  startBar: 0,
  lengthBars: 1,
  agent: false,
  data: { kind: "midi", notes: [], ...(controllers ? { controllers } : {}) },
});

describe("controller lane", () => {
  it("names lanes and the parameters the commands take", () => {
    expect(laneTitle(MOD)).toBe("Mod Wheel (CC1)");
    expect(laneTitle({ kind: "cc", number: 74 })).toBe("CC74");
    expect(laneTitle(BEND)).toBe("Pitch Bend");
    expect(laneParams(MOD)).toEqual({ kind: "cc", number: 1 });
    expect(laneParams(BEND)).toEqual({ kind: "bend" });
    expect(laneTitle({ ...BEND, channel: 3 })).toBe("Pitch Bend · Ch 4");
    expect(laneParams({ ...MOD, channel: 3 })).toEqual({
      kind: "cc",
      number: 1,
      channel: 3,
    });
    expect(sameLane(MOD, { ...MOD, channel: 0 })).toBe(true);
    expect(sameLane(MOD, { ...MOD, channel: 2 })).toBe(false);
    expect(laneOf({ ...point("a", "cc", 0, 5, 1), channel: 2 })).toEqual({
      kind: "cc",
      number: 1,
      channel: 2,
    });
    expect(parseCcNumber("CC 74")).toBe(74);
    expect(parseCcNumber("cc7")).toBe(7);
    expect(parseCcNumber("120")).toBeNull();
    expect(parseCcNumber("wheel")).toBeNull();
  });

  it("maps values to heights and back across each lane's range", () => {
    for (const [lane, values] of [
      [MOD, [0, 1, 64, 126, 127]],
      [BEND, [-8192, -4096, 0, 4096, 8191]],
      [{ kind: "pressure" } as Lane, [0, 127]],
    ] as const) {
      for (const value of values)
        expect(valueAtY(lane, yOfValue(lane, value, 96), 96)).toBe(value);
    }
    // Above the top and below the bottom clamp to the range.
    expect(valueAtY(MOD, -40, 96)).toBe(127);
    expect(valueAtY(MOD, 400, 96)).toBe(0);
    // The bend lane snaps to centre near its middle.
    expect(valueAtY(BEND, yOfValue(BEND, 60, 96), 96)).toBe(0);
    expect(yOfValue(BEND, 0, 96)).toBeCloseTo(48, 0);
  });

  it("finds a lane's points in time order and the lanes a clip uses", () => {
    const c = clip([
      point("b", "cc", 2, 90, 1),
      point("bend", "bend", 1, 100),
      point("a", "cc", 0, 10, 1),
      point("pedal", "cc", 3, 127, 64),
    ]);
    expect(lanePoints(c, MOD).map((p) => p.id)).toEqual(["a", "b"]);
    expect(lanePoints(clip(), MOD)).toEqual([]);
    expect(lanesInClip(c)).toEqual([
      { kind: "cc", number: 1 },
      { kind: "bend" },
      { kind: "cc", number: 64 },
    ]);
  });

  it("grabs the nearest point within the grip", () => {
    const points = [point("a", "cc", 0, 0, 1), point("b", "cc", 1, 127, 1)];
    const y0 = yOfValue(MOD, 0, 96);
    expect(hitPoint(points, MOD, 40, 96, 2, y0 - 1)?.id).toBe("a");
    expect(hitPoint(points, MOD, 40, 96, 41, yOfValue(MOD, 127, 96))?.id).toBe(
      "b",
    );
    expect(hitPoint(points, MOD, 40, 96, 20, 48)).toBeNull();
  });

  it("turns a stroke into one point per grid step, filling gaps", () => {
    const stroke = strokePoints(
      [
        { beat: 0.1, value: 0 },
        { beat: 1.1, value: 100 },
      ],
      0.25,
      4,
    )!;
    expect(stroke.points.map((p) => p.beat)).toEqual([0, 0.25, 0.5, 0.75, 1]);
    expect(stroke.points[0]!.value).toBe(0);
    // The last sample lands in the last cell and wins it.
    expect(stroke.points[4]!.value).toBe(100);
    expect(stroke.points[2]!.value).toBe(40);
    expect(stroke.from).toBe(0);
    expect(stroke.to).toBe(1.25);
    // Nothing lands at or past the clip end.
    const back = strokePoints(
      [
        { beat: 3.9, value: 10 },
        { beat: 5, value: 10 },
      ],
      0.5,
      4,
    )!;
    expect(back.points.map((p) => p.beat)).toEqual([3.5]);
    expect(back.to).toBe(4);
    expect(strokePoints([{ beat: 6, value: 1 }], 0.25, 4)).toBeNull();
  });

  it("labels bend in semitones and the pedal as up or down", () => {
    expect(valueLabel(BEND, 4096)).toBe("+1.00 st");
    expect(valueLabel(BEND, -8192)).toBe("-2.00 st");
    expect(valueLabel({ kind: "cc", number: 64 }, 127)).toBe("down");
    expect(valueLabel(MOD, 12)).toBe("12");
  });
});
