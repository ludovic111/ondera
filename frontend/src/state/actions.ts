import {
  barsToBeats,
  beatsToBars,
  commands,
  snapBars,
  type Clip,
  type Session,
  type SessionStore,
} from "@ondera/core";
import { newId } from "./ids";
import type { Shortcut } from "./shortcuts";

/**
 * Named UI actions: what menus and keyboard shortcuts invoke. Each one reads
 * the store to resolve "the selection" or "the playhead", then dispatches
 * ordinary commands. Nothing here mutates state directly.
 */
export interface ActionDef {
  id: ActionId;
  label: string;
  shortcut?: Shortcut;
  enabled?: (state: Session, store: SessionStore) => boolean;
  checked?: (state: Session, store: SessionStore) => boolean;
  run: (store: SessionStore) => void;
}

export type ActionId =
  | "undo"
  | "redo"
  | "togglePlay"
  | "stop"
  | "record"
  | "cycle"
  | "returnToStart"
  | "rewind"
  | "forward"
  | "metronome"
  | "deleteSelection"
  | "duplicateClip"
  | "splitAtPlayhead"
  | "openInEditor"
  | "addAudioTrack"
  | "addMidiTrack"
  | "removeSelectedTrack"
  | "muteSelectedTrack"
  | "soloSelectedTrack"
  | "armSelectedTrack"
  | "quantizeRegion"
  | "cycleMonitorSelectedTrack"
  | "zoomIn"
  | "zoomOut"
  | "zoomToFit"
  | "followPlayhead"
  | "toggleAgentPanel"
  | "askAgent"
  | "stopAgent"
  | "editorPianoRoll"
  | "editorScore"
  | "editorStep"
  | "toolPointer"
  | "toolPencil"
  | "toolScissors"
  | "copy"
  | "cut"
  | "paste"
  | "duplicateTrack"
  | "transposeUp"
  | "transposeDown"
  | "transposeOctaveUp"
  | "transposeOctaveDown"
  | "toggleMixer"
  | "toggleControllerLane"
  | "commandPalette"
  | "showShortcuts"
  | "addMarker"
  | "nextMarker"
  | "previousMarker"
  | "cycleSection";

export const SNAP_DIVISIONS = [1, 2, 4, 8, 16, 32, 64] as const;

const selectedClip = (s: Session) =>
  s.view.selectedClipId
    ? (s.clips.find((c) => c.id === s.view.selectedClipId) ?? null)
    : null;
const selectedTrack = (s: Session) =>
  s.view.selectedTrackId
    ? (s.tracks.find((t) => t.id === s.view.selectedTrackId) ?? null)
    : null;
const selectedNote = (s: Session) => {
  if (!s.view.selectedNoteId || !s.view.editorClipId) return null;
  const clip = s.clips.find((c) => c.id === s.view.editorClipId);
  if (!clip || clip.data.kind !== "midi") return null;
  return clip.data.notes.find((n) => n.id === s.view.selectedNoteId) ?? null;
};

function define(defs: ActionDef[]): Record<ActionId, ActionDef> {
  const out = {} as Record<ActionId, ActionDef>;
  for (const d of defs) out[d.id] = d;
  return out;
}

export const actions = define([
  {
    id: "undo",
    label: "Undo",
    shortcut: { key: "z", meta: true },
    enabled: (_s, store) => store.canUndo(),
    run: (store) => store.dispatch(commands.history.undo({})),
  },
  {
    id: "redo",
    label: "Redo",
    shortcut: { key: "z", meta: true, shift: true },
    enabled: (_s, store) => store.canRedo(),
    run: (store) => store.dispatch(commands.history.redo({})),
  },
  {
    id: "togglePlay",
    label: "Play / Stop",
    shortcut: { key: "Space" },
    run: (store) => store.dispatch(commands.transport.togglePlay({})),
  },
  {
    id: "stop",
    label: "Stop",
    shortcut: { key: "0" },
    run: (store) => store.dispatch(commands.transport.stop({})),
  },
  {
    id: "record",
    label: "Record",
    shortcut: { key: "r" },
    checked: (s) => s.transport.recording,
    run: (store) =>
      store.dispatch(
        commands.transport.setRecording({
          recording: !store.getState().transport.recording,
        }),
      ),
  },
  {
    id: "cycle",
    label: "Cycle",
    shortcut: { key: "c" },
    checked: (s) => s.transport.cycle,
    run: (store) =>
      store.dispatch(
        commands.transport.setCycle({
          enabled: !store.getState().transport.cycle,
        }),
      ),
  },
  {
    id: "returnToStart",
    label: "Go to Beginning",
    shortcut: { key: "Enter" },
    run: (store) => store.dispatch(commands.transport.returnToStart({})),
  },
  {
    id: "rewind",
    label: "Rewind One Bar",
    shortcut: { key: "," },
    run: (store) => store.dispatch(commands.transport.nudge({ bars: -1 })),
  },
  {
    id: "forward",
    label: "Forward One Bar",
    shortcut: { key: "." },
    run: (store) => store.dispatch(commands.transport.nudge({ bars: 1 })),
  },
  {
    id: "metronome",
    label: "Metronome Click",
    shortcut: { key: "k" },
    checked: (s) => s.transport.metronome,
    run: (store) =>
      store.dispatch(
        commands.transport.setMetronome({
          enabled: !store.getState().transport.metronome,
        }),
      ),
  },
  {
    id: "deleteSelection",
    label: "Delete",
    shortcut: { key: "Backspace" },
    enabled: (s) => selectedNote(s) !== null || selectedClip(s) !== null,
    run: (store) => {
      const s = store.getState();
      const note = selectedNote(s);
      if (note && s.view.editorClipId) {
        store.dispatch(
          commands.note.remove({
            clipId: s.view.editorClipId,
            noteId: note.id,
          }),
        );
        return;
      }
      const clip = selectedClip(s);
      if (clip) store.dispatch(commands.clip.remove({ clipId: clip.id }));
    },
  },
  {
    id: "duplicateClip",
    label: "Duplicate Clip",
    shortcut: { key: "d", meta: true },
    enabled: (s) => selectedClip(s) !== null,
    run: (store) => {
      const clip = selectedClip(store.getState());
      if (clip)
        store.dispatch(
          commands.clip.duplicate({
            clipId: clip.id,
            newClipId: newId("clip"),
          }),
        );
    },
  },
  {
    id: "splitAtPlayhead",
    label: "Split Clip at Playhead",
    shortcut: { key: "t", meta: true },
    enabled: (s) => {
      const clip = selectedClip(s);
      if (!clip) return false;
      const bar = beatsToBars(
        s.transport.positionBeats,
        s.transport.timeSignature,
      );
      return bar > clip.startBar && bar < clip.startBar + clip.lengthBars;
    },
    run: (store) => {
      const s = store.getState();
      const clip = selectedClip(s);
      if (!clip) return;
      const bar = snapBars(
        beatsToBars(s.transport.positionBeats, s.transport.timeSignature),
        s.transport.snapDivision,
        s.transport.timeSignature,
      );
      if (bar <= clip.startBar || bar >= clip.startBar + clip.lengthBars)
        return;
      store.dispatch(
        commands.clip.split({
          clipId: clip.id,
          atBar: bar,
          newClipId: newId("clip"),
        }),
      );
    },
  },
  {
    id: "openInEditor",
    label: "Open in Editor",
    shortcut: { key: "e" },
    enabled: (s) => selectedClip(s)?.data.kind === "midi",
    run: (store) => {
      const clip = selectedClip(store.getState());
      if (clip?.data.kind === "midi")
        store.dispatch(commands.view.setEditorClip({ clipId: clip.id }));
    },
  },
  {
    id: "addAudioTrack",
    label: "New Audio Track",
    shortcut: { key: "a", meta: true, alt: true },
    run: (store) =>
      store.dispatch(
        commands.track.add({
          trackId: newId("track"),
          kind: "audio",
          ...insertIndex(store),
        }),
      ),
  },
  {
    id: "addMidiTrack",
    label: "New MIDI Track",
    shortcut: { key: "s", meta: true, alt: true },
    run: (store) =>
      store.dispatch(
        commands.track.add({
          trackId: newId("track"),
          kind: "midi",
          ...insertIndex(store),
        }),
      ),
  },
  {
    id: "removeSelectedTrack",
    label: "Delete Track",
    shortcut: { key: "Backspace", meta: true },
    enabled: (s) => selectedTrack(s) !== null,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track) store.dispatch(commands.track.remove({ trackId: track.id }));
    },
  },
  {
    id: "muteSelectedTrack",
    label: "Mute Track",
    shortcut: { key: "m" },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.mute ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track)
        store.dispatch(
          commands.track.setMute({ trackId: track.id, muted: !track.mute }),
        );
    },
  },
  {
    id: "soloSelectedTrack",
    label: "Solo Track",
    shortcut: { key: "s" },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.solo ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track)
        store.dispatch(
          commands.track.setSolo({ trackId: track.id, solo: !track.solo }),
        );
    },
  },
  {
    id: "armSelectedTrack",
    label: "Record-Arm Track",
    shortcut: { key: "a" },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.armed ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track)
        store.dispatch(
          commands.track.setArmed({ trackId: track.id, armed: !track.armed }),
        );
    },
  },
  {
    id: "quantizeRegion",
    label: "Quantize Region Notes",
    enabled: (s) => selectedClip(s)?.data.kind === "midi",
    run: (store) => {
      const clip = selectedClip(store.getState());
      if (clip) store.fire("clip.quantize", { clipId: clip.id });
    },
  },
  {
    id: "cycleMonitorSelectedTrack",
    label: "Input Monitoring: Off / Auto / On",
    shortcut: { key: "i" },
    enabled: (s) => selectedTrack(s)?.kind === "audio",
    checked: (s) => (selectedTrack(s)?.monitor ?? "off") !== "off",
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track?.kind === "audio")
        store.dispatch(
          commands.track.setMonitor({
            trackId: track.id,
            monitor: ({ off: "auto", auto: "on", on: "off" } as const)[
              track.monitor ?? "off"
            ],
          }),
        );
    },
  },
  {
    id: "zoomIn",
    label: "Zoom In",
    shortcut: { key: "=", meta: true },
    run: (store) => zoomAroundPlayhead(store, Math.SQRT2),
  },
  {
    id: "zoomOut",
    label: "Zoom Out",
    shortcut: { key: "-", meta: true },
    run: (store) => zoomAroundPlayhead(store, Math.SQRT1_2),
  },
  {
    id: "zoomToFit",
    label: "Zoom to Fit Session",
    shortcut: { key: "z" },
    // The host knows the lane width (the arrangement reports it), so a script can fit too.
    run: (store) => store.fire("view.fit"),
  },
  {
    id: "followPlayhead",
    label: "Follow Playhead",
    shortcut: { key: "f" },
    checked: (s) => s.view.followPlayhead,
    run: (store) =>
      store.dispatch(
        commands.view.setFollowPlayhead({
          enabled: !store.getState().view.followPlayhead,
        }),
      ),
  },
  {
    id: "toggleAgentPanel",
    label: "Agent Panel",
    shortcut: { key: "j", meta: true },
    checked: (s) => s.view.agentPanelOpen,
    run: (store) =>
      store.dispatch(
        commands.view.setAgentPanelOpen({
          open: !store.getState().view.agentPanelOpen,
        }),
      ),
  },
  {
    id: "askAgent",
    label: "Ask Agent About Selection…",
    shortcut: { key: "j", meta: true, shift: true },
    run: (store) => store.askAgent(),
  },
  {
    id: "stopAgent",
    label: "Stop Current Agent Action",
    enabled: (s) => s.agent.current !== null,
    run: (store) => store.dispatch(commands.agent.stopCurrent({})),
  },
  {
    id: "editorPianoRoll",
    label: "Piano Roll",
    checked: (s) => s.view.editorMode === "pianoRoll",
    run: (store) =>
      store.dispatch(commands.view.setEditorMode({ mode: "pianoRoll" })),
  },
  {
    id: "editorScore",
    label: "Score",
    checked: (s) => s.view.editorMode === "score",
    run: (store) =>
      store.dispatch(commands.view.setEditorMode({ mode: "score" })),
  },
  {
    id: "editorStep",
    label: "Step",
    checked: (s) => s.view.editorMode === "step",
    run: (store) =>
      store.dispatch(commands.view.setEditorMode({ mode: "step" })),
  },
  {
    id: "toolPointer",
    label: "Pointer Tool",
    shortcut: { key: "1" },
    checked: (s) => s.view.arrangeTool === "pointer",
    run: (store) =>
      store.dispatch(commands.view.setArrangeTool({ tool: "pointer" })),
  },
  {
    id: "toolPencil",
    label: "Pencil Tool",
    shortcut: { key: "2" },
    checked: (s) => s.view.arrangeTool === "pencil",
    run: (store) =>
      store.dispatch(commands.view.setArrangeTool({ tool: "pencil" })),
  },
  {
    id: "toolScissors",
    label: "Scissors Tool",
    shortcut: { key: "3" },
    checked: (s) => s.view.arrangeTool === "scissors",
    run: (store) =>
      store.dispatch(commands.view.setArrangeTool({ tool: "scissors" })),
  },
  ...editingActions(),
  ...markerActions(),
]);

/** Markers are the song's sections; the host names a new one and moves the playhead. */
function markerActions(): ActionDef[] {
  const bar = (s: Session) =>
    beatsToBars(s.transport.positionBeats, s.transport.timeSignature);
  const markerAtPlayhead = (s: Session) =>
    s.markers.some((m) => Math.abs(m.bar - playheadBar(s)) < 1e-6);
  return [
    {
      id: "addMarker",
      label: "Add Marker at Playhead",
      shortcut: { key: "m", shift: true },
      enabled: (s) => !markerAtPlayhead(s),
      run: (store) =>
        store.dispatch(
          commands.marker.add({ bar: playheadBar(store.getState()) }),
        ),
    },
    {
      id: "previousMarker",
      label: "Go to Previous Marker",
      shortcut: { key: "b", shift: true },
      enabled: (s) => s.markers.some((m) => m.bar < bar(s) - 1e-6),
      run: (store) => store.dispatch(commands.marker.previous({})),
    },
    {
      id: "nextMarker",
      label: "Go to Next Marker",
      shortcut: { key: "n", shift: true },
      enabled: (s) => s.markers.some((m) => m.bar > bar(s) + 1e-6),
      run: (store) => store.dispatch(commands.marker.next({})),
    },
    {
      id: "cycleSection",
      label: "Cycle Section at Playhead",
      shortcut: { key: "c", shift: true },
      enabled: (s) => s.markers.some((m) => m.bar <= bar(s) + 1e-6),
      run: (store) => store.dispatch(commands.marker.cycleSection({})),
    },
  ];
}

/** Semitone shift of the selected note, or of every note in the selected region. */
function transpose(store: SessionStore, semitones: number): void {
  const s = store.getState();
  const note = selectedNote(s);
  if (note && s.view.editorClipId) {
    store.dispatch(
      commands.note.update({
        clipId: s.view.editorClipId,
        noteId: note.id,
        pitch: Math.max(0, Math.min(127, note.pitch + semitones)),
      }),
    );
    return;
  }
  const clip = selectedClip(s);
  if (clip) store.fire("clip.transpose", { clipId: clip.id, semitones });
}
function canTranspose(s: Session): boolean {
  return selectedNote(s) !== null || selectedClip(s)?.data.kind === "midi";
}

function editingActions(): ActionDef[] {
  const step = (
    id: ActionId,
    label: string,
    key: string,
    shift: boolean,
    semitones: number,
  ): ActionDef => ({
    id,
    label,
    shortcut: { key, alt: true, shift },
    enabled: canTranspose,
    run: (store) => transpose(store, semitones),
  });
  return [
    {
      id: "copy",
      label: "Copy",
      shortcut: { key: "c", meta: true },
      enabled: (s) => selectedClip(s) !== null,
      run: (store) => {
        const clip = selectedClip(store.getState());
        if (clip) store.fire("clip.copy", { clipId: clip.id });
      },
    },
    {
      id: "cut",
      label: "Cut",
      shortcut: { key: "x", meta: true },
      enabled: (s) => selectedClip(s) !== null,
      run: (store) => {
        const clip = selectedClip(store.getState());
        if (clip) store.fire("clip.cut", { clipId: clip.id });
      },
    },
    {
      // The clipboard is the host's, so this also pastes what the CLI or an agent copied.
      // It picks the track: the selected one when its kind fits, else the clip's own.
      id: "paste",
      label: "Paste at Playhead",
      shortcut: { key: "v", meta: true },
      run: (store) =>
        store.fire("clip.paste", { bar: playheadBar(store.getState()) }),
    },
    {
      id: "duplicateTrack",
      label: "Duplicate Track",
      shortcut: { key: "d", meta: true, shift: true },
      enabled: (s) => selectedTrack(s) !== null,
      run: (store) => {
        const track = selectedTrack(store.getState());
        if (track)
          store.dispatch(commands.track.duplicate({ trackId: track.id }));
      },
    },
    step("transposeUp", "Transpose Up a Semitone", "ArrowUp", false, 1),
    step("transposeDown", "Transpose Down a Semitone", "ArrowDown", false, -1),
    step("transposeOctaveUp", "Transpose Up an Octave", "ArrowUp", true, 12),
    step(
      "transposeOctaveDown",
      "Transpose Down an Octave",
      "ArrowDown",
      true,
      -12,
    ),
    {
      id: "toggleMixer",
      label: "Mixer",
      shortcut: { key: "x" },
      checked: (_s, store) => store.ui.mixer ?? false,
      run: (store) =>
        store.fire("ui.showPanel", {
          panel: "mixer",
          visible: !store.ui.mixer,
        }),
    },
    {
      // Mod wheel, sustain, pitch bend and other controllers of the open MIDI region.
      id: "toggleControllerLane",
      label: "Controller Lane",
      shortcut: { key: "l" },
      checked: (_s, store) => store.ui.controllers ?? false,
      run: (store) =>
        store.fire("ui.showPanel", {
          panel: "controllers",
          visible: !store.ui.controllers,
        }),
    },
    {
      id: "commandPalette",
      label: "Command Palette…",
      shortcut: { key: "p", meta: true },
      run: (store) => store.setOverlay("palette", true),
    },
    {
      id: "showShortcuts",
      label: "Shortcuts and Help",
      shortcut: { key: "/", meta: true },
      run: (store) =>
        store.fire("ui.showPanel", { panel: "help", visible: true }),
    },
  ];
}

/** New tracks go right below the selected one. */
function insertIndex(store: SessionStore): { index?: number } {
  const s = store.getState();
  const i = s.tracks.findIndex((t) => t.id === s.view.selectedTrackId);
  return i >= 0 ? { index: i + 1 } : {};
}

/**
 * The lane viewport width is a fact about the screen, not the session, so
 * the arrangement registers it here for zoom actions to read.
 */
let laneViewportWidth = 800;
export function reportLaneViewportWidth(px: number): void {
  if (px > 0) laneViewportWidth = px;
}
function viewportWidth(_store: SessionStore): number {
  return laneViewportWidth;
}

function zoomAroundPlayhead(store: SessionStore, factor: number): void {
  const s = store.getState();
  const bar = beatsToBars(s.transport.positionBeats, s.transport.timeSignature);
  const px = (bar - s.view.scrollBars) * s.view.pixelsPerBar;
  const width = viewportWidth(store);
  const onScreen = px >= 0 && px <= width;
  store.dispatch(
    commands.view.zoomBy(
      onScreen
        ? { factor, anchorBar: bar, anchorPx: px }
        : {
            factor,
            anchorBar: s.view.scrollBars + width / 2 / s.view.pixelsPerBar,
            anchorPx: width / 2,
          },
    ),
  );
}

export function runAction(store: SessionStore, id: ActionId): void {
  const def = actions[id];
  if (def.enabled && !def.enabled(store.getState(), store)) return;
  def.run(store);
}

/** Playhead as a snapped bar, for "at playhead" operations. */
export function playheadBar(s: Session, snap = true): number {
  const bar = beatsToBars(s.transport.positionBeats, s.transport.timeSignature);
  return snap
    ? snapBars(bar, s.transport.snapDivision, s.transport.timeSignature)
    : bar;
}

export function locateToBar(store: SessionStore, bar: number): void {
  const s = store.getState();
  store.dispatch(
    commands.transport.setPosition({
      beats: Math.max(0, barsToBeats(bar, s.transport.timeSignature)),
    }),
  );
}
