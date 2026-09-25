import { expect, it } from "vitest";
import type { Session } from "@ondera/core";
import { size } from "../theme/tokens";
import { drawRuler } from "./ruler";

/** A context that records every rounded rectangle and ignores the rest. */
function recorder() {
  const rects: { y: number; h: number }[] = [];
  const ctx = new Proxy(
    {
      roundRect(_x: number, y: number, _w: number, h: number) {
        rects.push({ y, h });
      },
      measureText: (text: string) => ({ width: text.length * 6 }),
    } as Record<string, unknown>,
    {
      get: (target, key) =>
        key in target ? target[key as string] : () => undefined,
      set: () => true,
    },
  ) as unknown as CanvasRenderingContext2D;
  return { ctx, rects };
}

it("keeps the playhead's position bubble above the marker flags", () => {
  const state = {
    view: { pixelsPerBar: 80, scrollBars: 0 },
    transport: {
      positionBeats: 0,
      timeSignature: [4, 4],
      cycle: false,
      cycleStartBar: 0,
      cycleEndBar: 4,
    },
    markers: [{ id: "m1", bar: 0, name: "Intro" }],
    tracks: [],
    clips: [],
  } as unknown as Session;
  const { ctx, rects } = recorder();
  drawRuler(ctx, 800, size.ruler, state);
  const flags = rects.filter((r) => r.y === size.markerTop);
  const others = rects.filter((r) => r.y !== size.markerTop);
  expect(flags.length).toBeGreaterThan(0);
  expect(others.length).toBeGreaterThan(0);
  for (const r of others) expect(r.y + r.h).toBeLessThanOrEqual(size.markerTop);
});
