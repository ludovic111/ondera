// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "../state/native";
import { SessionProvider } from "../state/session";
import { PluginPanel, ValueField } from "./PluginPanel";
import type { Parameter } from "./plugin/PluginFace";

afterEach(cleanup);
HTMLDialogElement.prototype.show = function () {
  this.open = true;
};
HTMLDialogElement.prototype.showModal = function () {
  this.open = true;
};
HTMLDialogElement.prototype.close = function () {
  this.open = false;
};
const cutoff: Parameter = {
  id: 3,
  name: "Cutoff",
  min: 20,
  max: 20000,
  value: 1000,
  default: 1000,
  unit: "Hz",
  steps: 0,
  labels: [],
  logarithmic: true,
};

// Every keystroke was sent: clearing the field set the cutoff to 0 at once, and "5" on the
// way to "500" was out of range.
it("a typed value commits once, in range, and never from an empty field", () => {
  const onCommit = vi.fn();
  const { getByLabelText } = render(
    <ValueField parameter={cutoff} onCommit={onCommit} />,
  );
  const field = getByLabelText("Cutoff value") as HTMLInputElement;
  fireEvent.change(field, { target: { value: "" } });
  fireEvent.blur(field);
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.change(field, { target: { value: "5" } });
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.change(field, { target: { value: "500" } });
  fireEvent.keyDown(field, { key: "Enter" });
  expect(onCommit).toHaveBeenCalledWith(500);
  fireEvent.change(field, { target: { value: "5" } });
  fireEvent.blur(field);
  expect(onCommit).toHaveBeenLastCalledWith(20);
});

// The refresh closure kept the slot the window opened on, so after Move up/down the
// window re-read another plugin's parameters.
it("the plugin window re-reads the slot its plugin sits in now", async () => {
  vi.useFakeTimers();
  const asked: unknown[] = [];
  mock.invoke.mockImplementation(async (_command, args) => {
    if (args.method === "strip.parameters") {
      asked.push(args.params.slot);
      return { pluginId: "stock:Chorus", parameters: [] };
    }
    if (args.method === "preset.list") return { presets: [] };
    return {};
  });
  const store = new NativeStore();
  const doc = {
    ...store.getState(),
    strips: {},
    masterVolume: 0.75,
    automation: [],
  } as DocumentData;
  store.receive(doc);
  const panel = (slot: number) => (
    <SessionProvider store={store}>
      <PluginPanel id="insert-1" trackId="t" slot={slot} onClose={() => {}} />
    </SessionProvider>
  );
  const view = render(panel(2));
  view.rerender(panel(1));
  await act(async () => {
    store.receive({ ...doc, name: "Moved" });
    vi.advanceTimersByTime(200);
  });
  vi.useRealTimers();
  expect(asked.at(-1)).toBe(1);
});
