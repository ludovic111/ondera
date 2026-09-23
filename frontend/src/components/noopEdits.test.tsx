// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore, type DocumentData } from "../state/native";
import { SessionProvider } from "../state/session";
import { AutomationPanel } from "./AutomationPanel";

// Tabbing through the automation point fields (or clicking a point without moving it) sent
// automation.setPoint with the values it already had: an undo step that changed nothing.
let sent: string[];
beforeEach(() => {
  sent = [];
  HTMLDialogElement.prototype.show = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  mock.invoke.mockImplementation(async (_command, args) => {
    if (args.method === "automation.list")
      return {
        lanes: [
          {
            id: "lane",
            name: "Keys · Volume",
            enabled: true,
            interpolation: "linear",
            min: 0,
            max: 1,
            manualValue: 0.75,
            target: { kind: "trackVolume", trackId: "t" },
            points: [{ id: "p", beat: 4, value: 0.5 }],
          },
        ],
      };
    sent.push(args.method);
    return {};
  });
});
afterEach(cleanup);

it("leaving an automation field unchanged sends nothing", async () => {
  const store = new NativeStore();
  store.receive({
    ...store.getState(),
    strips: {},
    masterVolume: 0.75,
    automation: [],
  } as DocumentData);
  const view = render(
    <SessionProvider store={store}>
      <AutomationPanel onClose={() => {}} />
    </SessionProvider>,
  );
  await act(async () => {});
  const [beat, value] = [
    ...view.container.querySelectorAll<HTMLInputElement>(
      ".automation-points input",
    ),
  ];
  fireEvent.blur(beat!);
  fireEvent.blur(value!);
  fireEvent.change(beat!, { target: { value: "" } });
  fireEvent.blur(beat!);
  await act(async () => {});
  expect(sent.filter((m) => m.startsWith("automation."))).toEqual([]);
  fireEvent.change(value!, { target: { value: "0.8" } });
  fireEvent.blur(value!);
  await act(async () => {});
  expect(sent).toContain("automation.setPoint");
});
