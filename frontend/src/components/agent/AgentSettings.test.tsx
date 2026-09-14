// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
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
import { NativeStore } from "../../state/native";
import { SessionProvider } from "../../state/session";
import { AgentSettings } from "./AgentSettings";
import type { Params } from "../../state/native";
const show = (settings: Params) => {
  const store = new NativeStore();
  const refresh = vi.fn(async () => {});
  mock.invoke
    .mockReset()
    .mockResolvedValue({
      provider: settings.provider,
      state: "configured",
      message: "Configured only.",
    });
  render(
    <SessionProvider store={store}>
      <AgentSettings
        settings={settings}
        refresh={refresh}
        onStartChat={vi.fn()}
      />
    </SessionProvider>,
  );
};
afterEach(cleanup);
describe("agent setup", () => {
  it("shows only the relevant provider fields and never puts a saved key in an input", async () => {
    show({ provider: "openai", openaiApiKey: "••••1234" });
    const key = screen.getByLabelText("API key") as HTMLInputElement;
    expect(key.value).toBe("");
    expect(key.placeholder).toContain("Key saved");
    expect(screen.queryByLabelText("Server address")).toBeNull();
    expect(screen.queryByLabelText(/Companion executable/)).toBeNull();
    expect(await screen.findByText("Ready to try")).toBeTruthy();
    expect(screen.queryByText("Account connected")).toBeNull();
  });
  it("saves the key only explicitly, with whitespace trimmed", async () => {
    show({ provider: "openai" });
    fireEvent.change(screen.getByLabelText("API key"), {
      target: { value: " test-key " },
    });
    expect(
      mock.invoke.mock.calls.some(([method]) => method === "daw_command"),
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Save connection" }));
    await waitFor(() =>
      expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
        method: "settings.set",
        params: { path: "agent.openaiApiKey", value: "test-key" },
      }),
    );
  });
  it("prevents switching service while connection edits are unsaved", async () => {
    show({
      provider: "compatible",
      compatibleBaseUrl: "http://localhost:1234/v1",
      model: "local",
    });
    fireEvent.change(screen.getByLabelText("Model name"), {
      target: { value: "another-model" },
    });
    expect(
      (screen.getByLabelText("AI service") as HTMLSelectElement).disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Check connection",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await screen.findByText("Ready to try");
  });
});

describe("connection feedback", () => {
  it.each([
    ["HTTP 429", "usage limit"],
    ["401 Unauthorized", "verify your account"],
    ["Connection refused", "could not reach"],
  ])(
    "explains %s without requiring developer vocabulary",
    async (reason, expected) => {
      const { agentErrorMessage } = await import("./connection");
      expect(agentErrorMessage(reason, false)).toContain(expected);
    },
  );
});
