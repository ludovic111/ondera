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

/**
 * A menu row for an action, resolving enabled/checked against current state, or against
 * `state` when the caller knows better: a context menu selects what was clicked, but that
 * selection reaches the store only after the menu is built.
 */
export function actionItem(
  store: SessionStore,
  id: ActionId,
  label?: string,
  state: Session = store.getState(),
): MenuEntry {
  const def = actions[id];
  return {
    label: label ?? def.label,
    ...(def.shortcut ? { shortcut: formatShortcut(def.shortcut) } : {}),
    disabled: def.enabled ? !def.enabled(state, store) : false,
    checked: def.checked ? def.checked(state, store) : false,
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
  const s = store.getState();
  const selected = s.clips.find((c) => c.id === s.view.selectedClipId);
  const file = (label: string, action: string) =>
    call(label, "web.file", { action });
  const panel = (label: string, name: string, section?: string) =>
    call(label, "ui.showPanel", {
      panel: name,
      visible: true,
      ...(section ? { section } : {}),
    });
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
        {
          label: "Import MIDI…",
          onSelect: () => void store.importMidiFile().catch(() => {}),
        },
        file("Export audio…", "export"),
        {
          label: "Export MIDI…",
          onSelect: () => void store.exportMidiFile().catch(() => {}),
        },
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
        a("cut"),
        a("copy"),
        a("paste"),
        a("duplicateClip", "Duplicate region"),
        a("splitAtPlayhead"),
        a("deleteSelection"),
        separator,
        a("quantizeRegion", "Quantize region notes"),
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
          // Every tool but Repeat rewrites notes, so it needs a MIDI region.
          disabled: !(method === "clip.repeat"
            ? selected
            : selected?.data.kind === "midi"),
          onSelect: () =>
            store.fire(String(method), {
              ...(params as Record<string, unknown>),
              clipId: store.getState().view.selectedClipId,
            }),
        })),
        separator,
        a("transposeUp"),
        a("transposeDown"),
        a("transposeOctaveUp"),
        a("transposeOctaveDown"),
      ];
    case "Track":
      return [
        a("addMidiTrack", "Add instrument track"),
        a("addAudioTrack", "Add audio track"),
        a("duplicateTrack"),
        a("removeSelectedTrack"),
        separator,
        a("muteSelectedTrack"),
        a("soloSelectedTrack"),
        a("armSelectedTrack"),
        a("cycleMonitorSelectedTrack"),
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
        call("Reconnect output", "audio.reconnect"),
        panel("Output device…", "settings", "audio"),
        panel("Input device…", "settings", "audio"),
        panel("MIDI input…", "settings", "audio"),
        {
          ...call("Musical typing", "ui.musicalTyping", {
            enabled: !store.ui.musicalTyping,
          }),
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
        a("askAgent"),
        a("stopAgent", "Stop agent"),
        panel("Agent settings…", "settings", "agent"),
      ];
    case "View":
      return [
        a("commandPalette"),
        a("toggleMixer"),
        a("toggleControllerLane"),
        panel("Automation", "automation"),
        separator,
        a("followPlayhead"),
        a("zoomToFit", "Fit session"),
        a("zoomIn"),
        a("zoomOut"),
        separator,
        a("addMarker"),
        a("previousMarker"),
        a("nextMarker"),
        a("cycleSection"),
      ];
    case "Help":
      return [
        a("showShortcuts"),
        call("Check for updates…", "app.checkUpdates"),
        call("Native plugin SDK…", "app.openGuide", { guide: "plugins" }),
        separator,
        { label: `Ondera ${store.version}`, disabled: true },
      ];
  }
}
export type { Session };
