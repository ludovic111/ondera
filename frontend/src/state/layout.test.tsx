// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore } from "./native";
import { SessionProvider } from "./session";
import { useKeyboardShortcuts } from "./keyboard";
import { matchesShortcut } from "./shortcuts";

afterEach(cleanup);
function Keyboard() {
  useKeyboardShortcuts();
  return null;
}

// Musical typing read KeyboardEvent.key, so on AZERTY the "A" key sat where QWERTY has Q and
// the piano was scrambled; ⌘⌥A and ⌘⌥S never matched on macOS, where Option composes "å".
it("musical typing plays by key position on any layout", () => {
  const store = new NativeStore();
  store.ui.musicalTyping = true;
  const fire = vi.spyOn(store, "fire");
  render(
    <SessionProvider store={store}>
      <Keyboard />
    </SessionProvider>,
  );
  // AZERTY: the key left of S types "q" but sits where QWERTY has A.
  fireEvent.keyDown(window, { key: "q", code: "KeyA" });
  expect(fire).toHaveBeenCalledWith("web.liveNote", { pitch: 60, on: true });
  fireEvent.keyUp(window, { key: "q", code: "KeyA" });
  expect(fire).toHaveBeenCalledWith("web.liveNote", { pitch: 60, on: false });
  // The key AZERTY labels "A" is QWERTY's Q: not a piano key.
  fire.mockClear();
  fireEvent.keyDown(window, { key: "a", code: "KeyQ" });
  expect(fire).not.toHaveBeenCalledWith(
    "web.liveNote",
    expect.objectContaining({ on: true }),
  );
});

it("Option shortcuts match the key under the composed character", () => {
  const press = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
  const newAudio = { key: "a", meta: true, alt: true };
  expect(
    matchesShortcut(
      press({ key: "å", code: "KeyA", metaKey: true, altKey: true }),
      newAudio,
    ),
  ).toBe(true);
  expect(
    matchesShortcut(
      press({ key: "ß", code: "KeyS", metaKey: true, altKey: true }),
      newAudio,
    ),
  ).toBe(false);
  // Without Option the character decides, so layouts keep their letters.
  expect(
    matchesShortcut(press({ key: "q", code: "KeyA", metaKey: true }), {
      key: "a",
      meta: true,
    }),
  ).toBe(false);
});
