import { expect, it, vi } from "vitest";
const mock = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mock.listen }));
import { NativeStore } from "./native";

// A form that shows its own error (agent connection, model picker, takes) also got the
// window's blocking error dialog for the same failure.
it("request leaves the error to the caller; run reports it", async () => {
  mock.invoke.mockRejectedValue("Invalid provider");
  const store = new NativeStore();
  await expect(store.request("agent.configure")).rejects.toBe(
    "Invalid provider",
  );
  await store.request("barrier").catch(() => {});
  expect(store.ui.error).toBeFalsy();
  await expect(store.run("agent.configure")).rejects.toBe("Invalid provider");
  await store.request("barrier").catch(() => {});
  expect(store.ui.error).toBe("Invalid provider");
});

// reportError set the error locally, and the next UI update from the host (a status line
// change) replaced the whole UI state, error included: the message flashed and vanished.
it("an error the window reported survives the host's next UI update", () => {
  const store = new NativeStore();
  store.receiveUi({ status: "Ready", error: null } as never);
  store.reportError("All eight inserts are occupied");
  store.receiveUi({ status: "Saving…", error: null } as never);
  expect(store.ui.error).toBe("All eight inserts are occupied");
  mock.invoke.mockResolvedValue({});
  store.dismissError();
  store.receiveUi({ status: "Saved", error: null } as never);
  expect(store.ui.error).toBeNull();
});
