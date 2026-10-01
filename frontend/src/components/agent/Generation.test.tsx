// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
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
import { GeneratePanel } from "./GeneratePanel";
import { ExternalAgents } from "./ExternalAgents";

afterEach(cleanup);
const calls = () =>
  mock.invoke.mock.calls
    .filter(([cmd]) => cmd === "daw_command")
    .map(([, args]) => args as { method: string; params: unknown });

const mount = (node: React.ReactNode) =>
  render(<SessionProvider store={new NativeStore()}>{node}</SessionProvider>);

it("generates a loop on the song's bars and lists it, without asking before a service is connected", async () => {
  let ready = false;
  mock.invoke.mockReset().mockImplementation(async (_cmd, args) => {
    switch (args?.method) {
      case "generate.services":
        return {
          chosen: "elevenlabs",
          services: [
            { id: "elevenlabs", label: "ElevenLabs", ready, makes: "" },
          ],
        };
      case "generate.list":
        return { sounds: [] };
      default:
        return { generated: { id: "x" } };
    }
  });
  mount(<GeneratePanel busy={false} />);
  expect(await screen.findByText("Connect a sound service")).toBeTruthy();
  const button = screen.getByRole("button", { name: "Generate loop" });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  cleanup();

  ready = true;
  mount(<GeneratePanel busy={false} />);
  await waitFor(() =>
    expect(screen.queryByText("Connect a sound service")).toBeNull(),
  );
  fireEvent.change(screen.getByLabelText("Describe the sound"), {
    target: { value: "  dusty drums  " },
  });
  fireEvent.change(screen.getByDisplayValue("4 bars"), {
    target: { value: "2" },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Generate loop" }));
  });
  await waitFor(() =>
    expect(calls().find((c) => c.method === "generate.audio")?.params).toEqual({
      prompt: "dusty drums",
      kind: "loop",
      service: "elevenlabs",
      bars: 2,
      followSong: true,
    }),
  );
});

it("asks for a song's length in seconds and turns a kept sound into an instrument", async () => {
  mock.invoke.mockReset().mockImplementation(async (_cmd, args) => {
    switch (args?.method) {
      case "generate.services":
        return {
          chosen: "fal",
          services: [{ id: "fal", label: "fal.ai", ready: true, makes: "" }],
        };
      case "generate.list":
        return {
          sounds: [
            {
              id: "1-felt",
              name: "Felt piano",
              description: "felt piano",
              kind: "instrument",
              service: "fal",
              seconds: 3,
              created: Date.now() / 1000,
            },
          ],
        };
      default:
        return {};
    }
  });
  mount(<GeneratePanel busy={false} />);
  await screen.findByText("Felt piano");
  fireEvent.click(screen.getByRole("radio", { name: "Song" }));
  fireEvent.change(screen.getByLabelText("Describe the sound"), {
    target: { value: "synthwave" },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Generate song" }));
  });
  await waitFor(() =>
    expect(calls().find((c) => c.method === "generate.audio")?.params).toEqual({
      prompt: "synthwave",
      kind: "song",
      service: "fal",
      seconds: 60,
      instrumental: true,
      followSong: true,
    }),
  );
  await act(async () => {
    fireEvent.click(await screen.findByRole("button", { name: "Keys" }));
  });
  await waitFor(() =>
    expect(calls().find((c) => c.method === "generate.place")?.params).toEqual({
      id: "1-felt",
      as: "instrument",
    }),
  );
});

it("offers a ready configuration for each outside agent and installs Cursor from its link", async () => {
  mock.invoke.mockReset().mockImplementation(async (_cmd, args) =>
    args?.method === "agent.mcp"
      ? {
          bridgeEnabled: false,
          clients: [
            {
              id: "claude-code",
              name: "Claude Code",
              how: "Run this once in a terminal.",
              text: "claude mcp add ryolune -- ryolune-mcp --live",
              file: null,
              link: false,
            },
            {
              id: "cursor",
              name: "Cursor",
              how: "Click Add to Cursor.",
              text: '{"mcpServers":{}}',
              file: "~/.cursor/mcp.json",
              link: true,
            },
          ],
        }
      : {},
  );
  mount(<ExternalAgents />);
  expect(
    await screen.findByText("claude mcp add ryolune -- ryolune-mcp --live"),
  ).toBeTruthy();
  // The bridge is off: say so and offer to turn it on.
  expect(screen.getByRole("button", { name: "Turn it on" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: /Add to/ })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "Cursor" }));
  expect(screen.getByText("~/.cursor/mcp.json")).toBeTruthy();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Add to Cursor" }));
  });
  await waitFor(() =>
    expect(calls().some((c) => c.method === "agent.openClient")).toBe(true),
  );
});
