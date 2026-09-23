// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "../../state/native";
import { SessionProvider } from "../../state/session";
import { useLaneInteraction } from "./useLaneInteraction";
import { size } from "../../theme/tokens";

afterEach(cleanup);

// A drag interrupted by the system (pointercancel, lost capture) left its ghost on screen
// and its state behind: the next release committed a move nobody made.
it("a cancelled clip drag is dropped, not committed on the next release", () => {
  mock.invoke.mockResolvedValue({});
  const store = new NativeStore();
  const state = store.getState();
  store.receive({
    ...state,
    tracks: [
      {
        id: "t",
        name: "Keys",
        kind: "midi",
        color: "#6ab3fd",
        mute: false,
        solo: false,
        armed: false,
        volume: 0.75,
        pan: 0,
      },
    ],
    clips: [
      {
        id: "c",
        trackId: "t",
        name: "Hook",
        startBar: 0,
        lengthBars: 4,
        data: { kind: "midi", notes: [] },
      },
    ],
    view: { ...state.view, pixelsPerBar: 40, scrollBars: 0 },
    strips: {},
    masterVolume: 0.75,
    automation: [],
  } as unknown as DocumentData);
  const dispatch = vi.spyOn(store, "dispatch");
  const lane = document.createElement("div");
  lane.setPointerCapture = () => {};
  lane.getBoundingClientRect = () => new DOMRect(0, 0, 800, 400);
  const at = (x: number) =>
    ({
      button: 0,
      clientX: x,
      clientY: size.trackRow / 2,
      pointerId: 1,
      altKey: false,
      currentTarget: lane,
    }) as never;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <SessionProvider store={store}>{children}</SessionProvider>
  );
  const { result } = renderHook(() => useLaneInteraction(), { wrapper });
  act(() => result.current.onPointerDown(at(40)));
  act(() => result.current.onPointerMove(at(200)));
  expect(result.current.overlay.ghost).toBeDefined();
  act(() => result.current.onPointerCancel(at(200)));
  expect(result.current.overlay.ghost).toBeUndefined();
  dispatch.mockClear();
  act(() => result.current.onPointerUp(at(200)));
  expect(dispatch).not.toHaveBeenCalled();
});
