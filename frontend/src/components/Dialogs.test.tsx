// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
const mock = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore } from "../state/native";
import { SessionProvider } from "../state/session";
import { Dialogs } from "./Dialogs";
let store: NativeStore;
beforeEach(() => {
  store = new NativeStore();
  HTMLDialogElement.prototype.show = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  mock.invoke.mockReset().mockImplementation(async (command, args) => {
    if (args?.method === "agent.models") return [];
    if (args?.method === "agent.connection")
      return {
        provider: "openai",
        state: "configured",
        message: "Key available",
      };
    if (args?.method === "settings.get")
      return {
        agent: { provider: "openai" },
        general: {
          checkUpdatesOnStart: false,
          installUpdatesAutomatically: false,
          confirmBeforeQuit: true,
        },
      };
    if (args?.method === "audio.devices")
      return { inputs: [], outputs: [], midiInputs: [] };
    return {};
  });
});
afterEach(cleanup);
const show = () =>
  render(
    <SessionProvider store={store}>
      <Dialogs />
    </SessionProvider>,
  );
describe("settings and export dialogs", () => {
  it("edits real update preferences in the Updates section", async () => {
    store.ui.settings = true;
    store.ui.settingsSection = "updates";
    show();
    fireEvent.click(await screen.findByLabelText("Check Updates On Start"));
    await waitFor(() =>
      expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
        method: "settings.set",
        params: { path: "general.checkUpdatesOnStart", value: true },
      }),
    );
    expect(screen.queryByLabelText("Confirm Before Quit")).toBeNull();
  });
  it("asks before closing unsaved agent connection details", async () => {
    store.ui.settings = true;
    store.ui.settingsSection = "agent";
    show();
    fireEvent.change(await screen.findByLabelText("API key"), {
      target: { value: "unsaved-test-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));
    expect(
      screen.getByText("Discard unsaved agent connection changes?"),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    expect((screen.getByLabelText("API key") as HTMLInputElement).value).toBe(
      "unsaved-test-key",
    );
    expect(
      screen.queryByText("Discard unsaved agent connection changes?"),
    ).toBeNull();
  });
  it("keeps an empty stem export out of the native file picker", () => {
    store.ui.export = true;
    show();
    fireEvent.click(screen.getByLabelText("Export track stems"));
    const button = screen.getByRole("button", {
      name: "Export…",
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(
      screen.getByText("Select at least one track to export as a stem."),
    ).toBeTruthy();
    fireEvent.click(button);
    expect(
      mock.invoke.mock.calls.some(([method]) => method === "daw_pick"),
    ).toBe(false);
  });
  it("asks for support once, after the third export", async () => {
    const general: Record<string, unknown> = { exportsCompleted: 1 };
    mock.invoke.mockImplementation(async (command, args) => {
      if (command === "daw_pick") return "/tmp/song.wav";
      if (args?.method === "settings.get") return { general: { ...general } };
      if (args?.method === "settings.set") {
        general[String(args.params.path).split(".")[1]] = args.params.value;
        return {};
      }
      if (args?.method === "session.exportAudio")
        return { path: "/tmp/song.wav", seconds: 4 };
      return {};
    });
    store.ui.export = true;
    show();
    const ask = () =>
      screen.queryByRole("button", { name: "Pay what you want…" });
    fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    await waitFor(() => expect(general.exportsCompleted).toBe(2));
    expect(ask()).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Export…" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Pay what you want…" }),
    );
    expect(general).toEqual({ exportsCompleted: 3, supportAsked: true });
    await waitFor(() =>
      expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
        method: "app.openGuide",
        params: { guide: "support" },
      }),
    );
    expect(ask()).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Export…" }));
    await waitFor(() => expect(general.exportsCompleted).toBe(4));
    expect(ask()).toBeNull();
  });
});
