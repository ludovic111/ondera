import { describe, expect, it } from "vitest";
import { describeTool } from "./toolSteps";

const step = (name: string, args: Record<string, unknown>) =>
  describeTool({ name, args, ok: true, result: null });

describe("describeTool", () => {
  it("reads the registry's own parameter names", () => {
    expect(step("track.setMute", { trackId: "t", muted: false })).toBe(
      "Unmuted a track",
    );
    expect(step("track.setMute", { trackId: "t", muted: true })).toBe(
      "Muted a track",
    );
  });
  it("tells a bypass from a cleared slot", () => {
    expect(step("strip.setInsert", { slot: 0, bypassed: true })).toBe(
      "Bypassed an effect",
    );
    expect(step("strip.setInsert", { slot: 0 })).toBe("Cleared an insert");
    expect(step("strip.setInsert", { slot: 0, effect: "Chorus" })).toBe(
      "Inserted “Chorus”",
    );
  });
});
