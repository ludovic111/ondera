import type { Session, SessionStore } from "@ondera/core";
import { actions, runAction, type ActionId } from "./actions";
import { formatShortcut } from "./shortcuts";

export interface MenuItem {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  checked?: boolean;
  onSelect?: () => void;
  separator?: false;
}

export type MenuEntry = MenuItem | { separator: true };

export const separator: MenuEntry = { separator: true };

/** A menu row for an action, resolving enabled/checked against current state. */
export function actionItem(
  store: SessionStore,
  id: ActionId,
  label?: string,
): MenuEntry {
  const def = actions[id];
  const state = store.getState();
  return {
    label: label ?? def.label,
    ...(def.shortcut ? { shortcut: formatShortcut(def.shortcut) } : {}),
    disabled: def.enabled ? !def.enabled(state, store) : false,
    checked: def.checked ? def.checked(state) : false,
    onSelect: () => runAction(store, id),
  };
}

export const MENU_TITLES = [
  "File",
  "Edit",
  "Track",
  "Mix",
  "Agent",
  "View",
  "Help",
] as const;
export type MenuTitle = (typeof MENU_TITLES)[number];
export function buildMenu(title: MenuTitle, store: SessionStore): MenuEntry[] {
  const a = (id: ActionId, label?: string) => actionItem(store, id, label);
  const call = (
    label: string,
    method: string,
    params: Record<string, unknown> = {},
  ): MenuItem => ({ label, onSelect: () => store.fire(method, params) });
  const file = (label: string, action: string) =>
    call(label, "web.file", { action });
  const panel = (label: string, name: string) =>
    call(label, "ui.showPanel", { panel: name, visible: true });
  switch (title) {
    case "File":
      return [
        file("New session", "new"),
        file("Open…", "open"),
        file("Open demo", "demo"),
        separator,
        file("Save", "save"),
        call("Save as…", "web.file", { action: "save", saveAs: true }),
        file("Import audio…", "import"),
        file("Import MIDI…", "importMidi"),
        file("Export audio…", "export"),
        file("Export MIDI…", "exportMidi"),
        separator,
        panel("Recover session…", "recovery"),
        separator,
        panel("Settings…", "settings"),
        separator,
        file("Quit", "quit"),
      ];
    case "Edit":
      return [
        a("undo"),
        a("redo"),
        separator,
        a("duplicateClip", "Duplicate region"),
        a("splitAtPlayhead"),
        a("deleteSelection"),
        separator,
        call("Quantize region notes", "web.quantize"),
        ...[
          [
            "Humanize timing & velocity",
            "clip.humanize",
            { timingMs: 10, velocity: 8, seed: 1 },
          ],
          ["Velocity crescendo", "clip.velocityRamp", { from: 55, to: 110 }],
          ["Velocity diminuendo", "clip.velocityRamp", { from: 110, to: 55 }],
          ["Legato notes", "clip.legato", {}],
          ["Reverse MIDI phrase", "clip.reverseMidi", {}],
          ["Fit to C major", "clip.fitScale", { root: 0, scale: "major" }],
          ["Fit to C minor", "clip.fitScale", { root: 0, scale: "minor" }],
          ["Repeat region × 4", "clip.repeat", { count: 3 }],
        ].map(([label, method, params]) => ({
          label: String(label),
          disabled: !store.getState().view.selectedClipId,
          onSelect: () =>
            store.fire(String(method), {
              ...(params as Record<string, unknown>),
              clipId: store.getState().view.selectedClipId,
            }),
        })),
        ...[1, -1, 12, -12].map((n) =>
          call(
            `Transpose region ${n > 0 ? "up" : "down"}${Math.abs(n) === 12 ? " an octave" : ""}`,
            "web.transpose",
            { semitones: n },
          ),
        ),
      ];
    case "Track":
      return [
        a("addMidiTrack", "Add instrument track"),
        a("addAudioTrack", "Add audio track"),
        separator,
        ...[
          ["Show master strip", "master"],
          ["Show reverb bus (A)", "bus-a"],
          ["Show delay bus (B)", "bus-b"],
        ].map(([label, id]) =>
          call(label, "ui.showPanel", { panel: id, visible: true }),
        ),
      ];
    case "Mix":
      return [
        file("Save recovered take…", "recoverTake"),
        call("Reconnect output", "web.reconnect"),
        panel("Output device…", "settings"),
        panel("Input device…", "settings"),
        panel("MIDI input…", "settings"),
        {
          ...call("Musical typing", "web.typing"),
          checked: store.ui.musicalTyping,
        },
        separator,
        {
          label: "Rescan plugins",
          onSelect: () => {
            void store
              .run("plugin.scan")
              .then(() => store.refreshPlugins())
              .catch(store.reportError);
          },
        },
      ];
    case "Agent":
      return [
        a("toggleAgentPanel", "Show agent panel"),
        call("Stop", "agent.stop"),
        panel("Agent settings…", "settings"),
      ];
    case "View":
      return [
        panel("Automation", "automation"),
        separator,
        a("followPlayhead"),
        a("zoomToFit", "Fit session"),
        a("zoomIn"),
        a("zoomOut"),
      ];
    case "Help":
      return [
        panel("Working in Ondera", "help"),
        call("Check for updates…", "app.checkUpdates"),
        call("Native plugin SDK…", "web.sdk"),
        separator,
        { label: `Ondera ${store.version}`, disabled: true },
      ];
  }
}
export type { Session };
