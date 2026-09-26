// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
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
import { AgentPanel, threadItems } from "./AgentPanel";
import { ModelSelector } from "./ModelSelector";
import { AgentMessage, humanStatus } from "./AgentMessage";
import { SessionProvider } from "../../state/session";
import { NativeStore } from "../../state/native";
let store: NativeStore;
beforeEach(() => {
  store = new NativeStore();
  store.agent.status.provider = "codex";
  Element.prototype.scrollIntoView = vi.fn();
  mock.invoke.mockReset().mockImplementation(async (method, args) => {
    if (args?.method === "agent.connection")
      return { provider: "codex", state: "signedIn", message: "Connected" };
    if (args?.method === "agent.models")
      return [
        {
          provider: "codex",
          label: "Account",
          models: [
            {
              id: "future-model",
              name: "Future model",
              efforts: ["low", "xhigh"],
            },
          ],
          error: null,
        },
        {
          provider: "compatible",
          label: "Local server",
          models: [{ id: "qwen-test", name: "Qwen test", efforts: [] }],
          error: null,
        },
      ];
    if (args?.method === "take.list") return { active: null, takes: [] };
    return {};
  });
});
afterEach(cleanup);
const panel = () =>
  render(
    <SessionProvider store={store}>
      <AgentPanel />
    </SessionProvider>,
  );
it("renders Markdown without executing HTML or fetching generated images", () => {
  const view = render(
    <AgentMessage
      text={
        '**Warm bass**\n\n- Kick\n- Snare\n\n<img src="https://tracker.invalid/a">\n\n![x](https://tracker.invalid/b)\n\n<script>alert(1)</script>'
      }
      streaming
    />,
  );
  expect(view.container.querySelector("strong")?.textContent).toBe("Warm bass");
  expect(view.container.querySelectorAll("li")).toHaveLength(2);
  expect(view.container.querySelectorAll("img,script")).toHaveLength(0);
  expect(screen.getByLabelText("Writing response")).toBeTruthy();
});
it("keeps technical commands out of chat and makes them available in Changes", async () => {
  store.agent.transcript.entries = [
    { role: "assistant", text: "**A warm beat.**" },
    {
      role: "tool",
      text: "",
      tool: {
        name: "session.inspect",
        args: {},
        result: { tracks: 2 },
        ok: true,
      },
    },
  ];
  store.agent.status.running = true;
  store.agent.status.status = "Running session.inspect…";
  store.agent.changes = [
    {
      sequence: 1,
      title: "session.inspect",
      detail: "session.inspect",
      output: '{"tracks":2}',
      succeeded: true,
      running: false,
      mutated: false,
      applied: false,
    },
  ];
  panel();
  await screen.findByText("A warm beat.");
  expect(screen.queryByText("session.inspect")).toBeNull();
  await screen.findByText("Checking your project…");
  // Reading the project is one quiet line in the chat, never a command name.
  expect(screen.getByText("Looked at your project")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /Changes/ }));
  expect(screen.getAllByText("session.inspect").length).toBeGreaterThan(0);
});
it("supports slash keyboard selection without sending an inference request", async () => {
  panel();
  const input = screen.getByRole("textbox", { name: "Message to agent" });
  fireEvent.change(input, { target: { value: "/diag" } });
  expect(screen.getByRole("option", { name: /diagnose/ })).toBeTruthy();
  fireEvent.keyDown(input, { key: "Enter" });
  expect((input as HTMLTextAreaElement).value).toContain(
    "Diagnose this problem",
  );
  // Looking up models and the connection is not talking to the agent.
  expect(
    mock.invoke.mock.calls.some(
      ([m, args]) =>
        m === "daw_command" &&
        !["agent.models", "agent.connection"].includes(args?.method),
    ),
  ).toBe(false);
  fireEvent.change(input, { target: { value: "/rhythm" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(screen.getByRole("region", { name: "Rhythm Lab" })).toBeTruthy();
  await act(async () => {});
});
it("loads account models and only their advertised thinking modes, then saves atomically", async () => {
  render(
    <SessionProvider store={store}>
      <ModelSelector provider="codex" model="" disabled={false} />
    </SessionProvider>,
  );
  const summary = screen.getByLabelText("Choose agent model");
  const details = summary.closest("details")!;
  await act(async () => {
    details.open = true;
    fireEvent(details, new Event("toggle"));
  });
  fireEvent.click(await screen.findByRole("radio", { name: /Future model/ }));
  expect(screen.queryByRole("option", { name: "High" })).toBeNull();
  fireEvent.change(screen.getByLabelText("Reasoning effort"), {
    target: { value: "xhigh" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Use this model" }));
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
      method: "agent.configure",
      params: {
        provider: "codex",
        model: "future-model",
        reasoningEffort: "xhigh",
      },
    }),
  );
});
it("creates a protected original before a variation", async () => {
  panel();
  fireEvent.click(screen.getByRole("button", { name: "Takes A/B" }));
  fireEvent.click(
    await screen.findByRole("button", { name: "Create & explore" }),
  );
  await waitFor(() =>
    expect(mock.invoke).toHaveBeenCalledWith("daw_command", {
      method: "take.create",
      params: { name: "Variation 1" },
    }),
  );
  const creates = mock.invoke.mock.calls.filter(
    ([, args]) => args?.method === "take.create",
  );
  expect(creates.map(([, args]) => args.params.name)).toEqual([
    "Original",
    "Variation 1",
  ]);
  expect(
    (
      screen.getByRole("textbox", {
        name: "Message to agent",
      }) as HTMLTextAreaElement
    ).value,
  ).toContain("original version is preserved");
});
it("maps unknown tool names and failures to human status", () => {
  expect(humanStatus("Running unknown_future_tool", true, false)).toBe(
    "Thinking…",
  );
  expect(humanStatus("HTTP 429", false, true)).toBe(
    "Could not finish this request",
  );
});

it("identifies compatible model makers without inventing logos for unknown providers", async () => {
  const { modelBrand, ProviderLogo } = await import("./ProviderLogo");
  expect(modelBrand("compatible", "Qwen/Qwen3-32B")?.name).toBe("Qwen");
  expect(modelBrand("compatible", "moonshotai/kimi-k2")?.name).toBe("Kimi");
  expect(modelBrand("compatible", "my-private-model")).toBeNull();
  render(<ProviderLogo provider="anthropic" />);
  expect(screen.getByAltText("Anthropic logo").getAttribute("src")).toBe(
    "/providers/claude-color.svg",
  );
});

it("shows what the agent changed in plain words and sends the selection along", async () => {
  store.agent.transcript.entries = [
    {
      role: "user",
      text: 'Add drums\n\n[Selected in the window: track "Keys" (trackId t1, instrument)]',
    },
    {
      role: "tool",
      text: "",
      tool: {
        name: "track_add",
        args: { kind: "midi", name: "Drums" },
        result: { id: "t2" },
        ok: true,
      },
    },
    {
      role: "tool",
      text: "",
      tool: {
        name: "clip.create",
        args: { notes: [1, 2, 3] },
        result: { error: "Track is full" },
        ok: false,
      },
    },
  ];
  panel();
  expect(await screen.findByText("Add drums")).toBeTruthy();
  expect(screen.getByText('about track "Keys"')).toBeTruthy();
  expect(screen.getByText("Added an instrument track “Drums”")).toBeTruthy();
  expect(screen.getByText("Created a region 3 notes")).toBeTruthy();
  expect(screen.getByText("Track is full")).toBeTruthy();
});
it("keys conversation items by entry id so trimming keeps each item's identity", () => {
  const tool = { name: "track.add", args: {}, ok: true, result: {} };
  const before = threadItems([
    { id: 7, role: "user", text: "a" },
    { id: 8, role: "tool", text: "", tool },
    { id: 9, role: "tool", text: "", tool },
    { id: 10, role: "assistant", text: "b" },
  ]);
  const after = threadItems([
    { id: 9, role: "tool", text: "", tool },
    { id: 10, role: "assistant", text: "b" },
  ]);
  expect(before.map((i) => i.key)).toEqual(["7", "8", "10"]);
  expect(after.map((i) => i.key)).toEqual(["9", "10"]);
});
