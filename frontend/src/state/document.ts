import type { SessionStore } from "@ondera/core";
export function newSession(store: SessionStore) {
  store.fire("web.file", { action: "new" });
}
export async function openSession(store: SessionStore) {
  await store.run("web.file", { action: "open" });
}
export async function saveSession(store: SessionStore, saveAs = false) {
  await store.run("web.file", { action: "save", saveAs });
}
export async function importAudioFiles(store: SessionStore) {
  await store.run("web.file", { action: "import" });
}
export async function bounceSession(store: SessionStore) {
  await store.run("web.file", { action: "export" });
}
