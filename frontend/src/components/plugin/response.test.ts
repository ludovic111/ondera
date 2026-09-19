import { describe, expect, it } from "vitest";
import {
  compressDb,
  envelopePoints,
  eqDb,
  filterDb,
  hzToX,
  xToHz,
} from "./response";
import { formatValue, fromPosition, toPosition } from "./PluginFace";

const flat = {
  lowGain: 0,
  lowFreq: 120,
  midGain: 0,
  midFreq: 1000,
  midQ: 0.8,
  highGain: 0,
  highFreq: 6000,
};

describe("plugin display maths", () => {
  it("draws a flat EQ as 0 dB and a boosted band at its gain", () => {
    for (const hz of [30, 440, 12000]) expect(eqDb(flat, hz)).toBeCloseTo(0, 6);
    expect(eqDb({ ...flat, midGain: 6 }, 1000)).toBeCloseTo(6, 1);
    // A shelf reaches its full gain well past the corner, half of it at the corner.
    expect(eqDb({ ...flat, lowGain: 12 }, 20)).toBeCloseTo(12, 0);
    expect(eqDb({ ...flat, lowGain: 12 }, 120)).toBeCloseTo(6, 0);
    expect(eqDb({ ...flat, highGain: -9 }, 19000)).toBeCloseTo(-9, 0);
  });

  it("matches the state-variable filter: -3 dB at cutoff when k = √2, 12 dB/oct", () => {
    const resonance = ((2 - Math.SQRT2) / 1.9) * 100;
    expect(filterDb(0, 1000, resonance, 1000)).toBeCloseTo(-3.01, 1);
    expect(
      filterDb(0, 1000, resonance, 8000) - filterDb(0, 1000, resonance, 4000),
    ).toBeCloseTo(-12, 0);
    expect(filterDb(1, 1000, resonance, 20)).toBeLessThan(-60);
    expect(filterDb(0, 1000, 100, 1000)).toBeCloseTo(20, 0);
  });

  it("compresses above the knee by the ratio", () => {
    expect(compressDb(-40, -18, 4)).toBeCloseTo(-40);
    expect(compressDb(-6, -18, 4)).toBeCloseTo(-18 + 12 / 4);
    expect(compressDb(-6, -18, 4, 3)).toBeCloseTo(-12);
  });

  it("keeps a short attack visible beside a long release", () => {
    const points = envelopePoints({
      attack: 5,
      decay: 200,
      sustain: 0.6,
      release: 3000,
    });
    expect(points[1]![0]).toBeGreaterThan(0.02);
    expect(points.at(-1)).toEqual([1, 0]);
    for (let i = 1; i < points.length; i++)
      expect(points[i]![0]).toBeGreaterThanOrEqual(points[i - 1]![0]);
  });

  it("round-trips the log frequency axis and log knobs", () => {
    expect(xToHz(hzToX(1000))).toBeCloseTo(1000);
    const cutoff = {
      id: 1,
      name: "Cutoff",
      min: 20,
      max: 20000,
      value: 1000,
      default: 1000,
      unit: "Hz",
      steps: 0,
      labels: [],
      logarithmic: true,
    };
    expect(toPosition(cutoff, 632.5)).toBeCloseTo(0.5, 2);
    expect(fromPosition(cutoff, toPosition(cutoff, 4321))).toBeCloseTo(4321);
    expect(formatValue(cutoff, 1000)).toBe("1.00 kHz");
    expect(formatValue({ ...cutoff, unit: ":1" }, 4)).toBe("4.00:1");
  });
});
