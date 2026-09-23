// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "../../state/native";
import { SessionProvider } from "../../state/session";
import { TrackHeader } from "./TrackHeader";

afterEach(cleanup);

// The menu selected the clicked track, but that selection reached the store only after the
// rows were judged, so Mute showed the previously selected track's tick.
it("a track's context menu reflects that track, not the previous selection", () => {
  mock.invoke.mockResolvedValue({});
  const store = new NativeStore();
  const state = store.getState();
  const base = {
    color: "#6ab3fd",
    armed: false,
    volume: 0.75,
    pan: 0,
    solo: false,
  };
  const muted = { ...base, id: "a", name: "Muted", kind: "midi", mute: true };
  const other = { ...base, id: "b", name: "Other", kind: "midi", mute: false };
  const doc = {
    ...state,
    tracks: [muted, other],
    view: { ...state.view, selectedTrackId: "b" },
    strips: {},
    masterVolume: 0.75,
    automation: [],
  } as unknown as DocumentData;
  store.receive(doc);
  const view = render(
    <SessionProvider store={store}>
      <TrackHeader track={store.getState().tracks[0]!} />
    </SessionProvider>,
  );
  fireEvent.contextMenu(view.container.firstElementChild!);
  const mute = [...document.querySelectorAll("[role=menuitem]")].find((e) =>
    e.textContent?.includes("Mute Track"),
  );
  expect(mute?.textContent).toContain("✓");
  const del = [...document.querySelectorAll("[role=menuitem]")].find((e) =>
    e.textContent?.includes("Delete Track"),
  );
  expect(del?.getAttribute("aria-disabled")).not.toBe("true");
});
