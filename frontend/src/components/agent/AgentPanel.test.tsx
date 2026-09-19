// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
const mock = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { NativeStore } from "../../state/native";
import { SessionProvider } from "../../state/session";
import { AgentPanel } from "./AgentPanel";
let store: NativeStore;
const renderPanel = () =>
  render(
    <SessionProvider store={store}>
      <AgentPanel />
    </SessionProvider>,
  );
beforeEach(() => {
  store = new NativeStore();
  store.agent.status.provider = "codex";
  Element.prototype.scrollIntoView = vi.fn();
  mock.invoke.mockReset().mockImplementation(async (method) => {
    if (method === "daw_agent_connection")
      return { provider: "codex", state: "signedIn", message: "Signed in." };
    return { ...store.agent.status, running: true };
  });
});
afterEach(cleanup);
describe("agent conversation", () => {
  it("fills a musical starter without sending it, then sends on Enter", async () => {
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: /Start a beat/ }));
    const input = screen.getByRole("textbox", { name: "Message to agent" });
    expect((input as HTMLTextAreaElement).value).toContain(
      "four-bar drum groove",
    );
    expect(
      mock.invoke.mock.calls.some(([method]) => method === "daw_command"),
    ).toBe(false);
    await waitFor(() =>
      expect(
        (screen.getByRole("button", { name: "Send" }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() =>
      expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
        method: "agent.send",
        params: { prompt: expect.stringContaining("four-bar drum groove") },
      }),
    );
  });
  it("keeps newlines and IME composition from sending a message", async () => {
    renderPanel();
    const input = screen.getByRole("textbox", { name: "Message to agent" });
    fireEvent.change(input, { target: { value: "Une mélodie" } });
    await waitFor(() =>
      expect(
        (screen.getByRole("button", { name: "Send" }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(input, { key: "Enter", repeat: true });
    expect(
      mock.invoke.mock.calls.some(([method]) => method === "daw_command"),
    ).toBe(false);
  });
  it("retains a draft after collapsing and reopening the panel", async () => {
    const view = renderPanel();
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "Do not lose my idea" },
    });
    view.unmount();
    renderPanel();
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "Do not lose my idea",
    );
    await act(async () => {});
  });
  it("opens the agent settings section when the companion is missing", async () => {
    mock.invoke.mockResolvedValue({
      provider: "codex",
      state: "missingCli",
      message: "Install the companion.",
    });
    renderPanel();
    fireEvent.click(
      await screen.findByRole("button", { name: "Set up agent" }),
    );
    await waitFor(() =>
      expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
        method: "ui.showPanel",
        params: { panel: "settings", section: "agent" },
      }),
    );
    expect(
      (screen.getByRole("button", { name: "Send" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });
  it("requires a deliberate confirmation to clear conversation and disables undo during work", async () => {
    store.agent.transcript.entries = [{ role: "user", text: "An idea" }];
    store.agent.changes = [
      {
        sequence: 1,
        title: "Add keys",
        detail: "One track",
        output: "{}",
        succeeded: true,
        running: false,
        mutated: true,
        applied: true,
      },
    ];
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "New conversation" }));
    expect(
      screen.getByText("Clear the conversation? Your music stays."),
    ).toBeTruthy();
    expect(
      mock.invoke.mock.calls.some(([method]) => method === "daw_command"),
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    store.agent.status.running = true;
    fireEvent.click(screen.getByRole("button", { name: /Changes · 1/ }));
    expect(
      (
        screen.getByRole("button", {
          name: "Undo from here",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await act(async () => {});
  });
});
