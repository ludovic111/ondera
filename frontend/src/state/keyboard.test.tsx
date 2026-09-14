// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore } from "./native";
import { SessionProvider } from "./session";
import { useKeyboardShortcuts } from "./keyboard";
let store: NativeStore;
function Keyboard() {
  useKeyboardShortcuts();
  return (
    <>
      <button>Control</button>
      <dialog open>
        <button>Dialog control</button>
      </dialog>
      <input aria-label="Edit" />
    </>
  );
}
beforeEach(() => {
  store = new NativeStore();
});
afterEach(cleanup);
describe("keyboard ownership", () => {
  it("does not play behind a dialog, while composing text, or on a focused button", () => {
    const dispatch = vi.spyOn(store, "dispatch");
    const { getByText, getByRole } = render(
      <SessionProvider store={store}>
        <Keyboard />
      </SessionProvider>,
    );
    fireEvent.keyDown(getByText("Control"), { key: " ", code: "Space" });
    fireEvent.keyDown(getByText("Dialog control"), { key: " ", code: "Space" });
    fireEvent.keyDown(getByRole("textbox"), { key: " ", code: "Space" });
    fireEvent.keyDown(window, { key: " ", code: "Space", isComposing: true });
    expect(dispatch).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: " ", code: "Space" });
    expect(dispatch).toHaveBeenCalledTimes(1);
  });
  it("blocks shortcuts during unsaved-work confirmation and releases notes when typing into a field", () => {
    const dispatch = vi.spyOn(store, "dispatch");
    const fire = vi.spyOn(store, "fire");
    const { getByRole } = render(
      <SessionProvider store={store}>
        <Keyboard />
      </SessionProvider>,
    );
    store.ui.prompt = "quit";
    fireEvent.keyDown(window, { key: " ", code: "Space" });
    expect(dispatch).not.toHaveBeenCalled();
    fireEvent.focusIn(getByRole("textbox"));
    expect(fire).toHaveBeenCalledWith("web.releaseKeys");
  });
});
