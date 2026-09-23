import { describe, expect, it } from "vitest";
import { roundRectPath } from "./paint";

describe("roundRectPath", () => {
  it("never passes a negative size or radius to roundRect", () => {
    const calls: number[][] = [];
    const ctx = {
      beginPath() {},
      roundRect(x: number, y: number, w: number, h: number, r: number) {
        // Canvas throws a RangeError on a negative radius.
        if (r < 0) throw new RangeError("negative radius");
        calls.push([x, y, w, h, r]);
      },
    } as unknown as CanvasRenderingContext2D;
    expect(() => roundRectPath(ctx, 10, 0, -1.5, 20, 4)).not.toThrow();
    expect(() => roundRectPath(ctx, 10, 0, 30, -2, 4)).not.toThrow();
    roundRectPath(ctx, 0, 0, 30, 20, 4);
    expect(calls).toEqual([
      [10, 0, 0, 20, 0],
      [10, 0, 30, 0, 0],
      [0, 0, 30, 20, 4],
    ]);
  });
});
