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
