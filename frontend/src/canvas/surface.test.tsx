// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { useCanvasSurface } from "./surface";
import { SessionProvider } from "../state/session";
import { NativeStore } from "../state/native";
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
it("paints current canvases for agent capture even when animation frames are suspended", () => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  const draw = vi.fn();
  const context = { setTransform: vi.fn(), clearRect: vi.fn() };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    context as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(300);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(200);
  vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);
  const cancel = vi
    .spyOn(window, "cancelAnimationFrame")
    .mockImplementation(() => {});
  function Canvas() {
    return <canvas ref={useCanvasSurface(draw)} />;
  }
  const view = render(
    <SessionProvider store={new NativeStore()}>
      <Canvas />
    </SessionProvider>,
  );
  expect(draw).not.toHaveBeenCalled();
  window.dispatchEvent(new Event("ondera:before-capture"));
  expect(draw).toHaveBeenCalledWith(context, 300, 200);
  expect(cancel).toHaveBeenCalledWith(1);
  view.unmount();
  draw.mockClear();
  window.dispatchEvent(new Event("ondera:before-capture"));
  expect(draw).not.toHaveBeenCalled();
});
