import { describe, it, expect } from "vitest";
import { exportProblem, exportSummary } from "./export";
const options = {
  range: true,
  start: 1,
  end: 5,
  tail: 3,
  stems: false,
  trackCount: 0,
};
describe("audio export", () => {
  it("accepts a valid exclusive range and mix export without a stem selection", () => {
    expect(exportProblem(options)).toBe("");
  });
  it.each([0, -1, NaN, Infinity])("rejects invalid start bar %s", (start) => {
    expect(exportProblem({ ...options, start })).not.toBe("");
  });
  it.each([0, 1, NaN, Infinity])(
    "rejects end bar %s before opening the picker",
    (end) => {
      expect(exportProblem({ ...options, end })).not.toBe("");
    },
  );
  it("never turns an empty stem selection into an all-track export", () => {
    expect(exportProblem({ ...options, stems: true })).toContain(
      "Select at least one",
    );
    expect(exportProblem({ ...options, stems: true, trackCount: 1 })).toBe("");
  });
  it("shows the real paths, duration, clipping and warnings from every stem", () => {
    const result = exportSummary({
      directory: "/music/stems",
      warnings: ["Stems ignore solo"],
      files: [
        {
          path: "/music/stems/keys.wav",
          seconds: 3.25,
          clippedSamples: 10,
          warnings: ["Tail shortened"],
        },
        { path: "/music/stems/drums.wav", seconds: 3.25, warnings: [] },
      ],
    });
    const ogg = exportSummary({
      path: "/music/mix.ogg",
      seconds: 2,
      kbps: 203.4,
      clippedSamples: 4,
    });
    expect(ogg).toContain("about 203 kbit/s");
    expect(ogg).toContain("4 samples are above full scale");
    expect(ogg).not.toContain("32-bit float");
    for (const text of [
      "2 stem files",
      "keys.wav",
      "drums.wav",
      "3.25 seconds",
      "10 samples clipped",
      "Tail shortened",
      "Stems ignore solo",
    ])
      expect(result).toContain(text);
  });
});
