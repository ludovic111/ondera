import { barsToBeats, beatsToBars, commands, snapBars, type Session, type SessionStore } from '@ondera/core';
import { newId } from './ids';
import type { Shortcut } from './shortcuts';

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
  checked?: (state: Session) => boolean;
  run: (store: SessionStore) => void;
}

export type ActionId =
  | 'undo'
  | 'redo'
  | 'togglePlay'
  | 'stop'
  | 'record'
  | 'cycle'
  | 'returnToStart'
  | 'rewind'
  | 'forward'
  | 'metronome'
  | 'deleteSelection'
  | 'duplicateClip'
  | 'splitAtPlayhead'
  | 'openInEditor'
  | 'addAudioTrack'
  | 'addMidiTrack'
  | 'removeSelectedTrack'
  | 'muteSelectedTrack'
  | 'soloSelectedTrack'
  | 'armSelectedTrack'
  | 'zoomIn'
  | 'zoomOut'
  | 'zoomToFit'
  | 'followPlayhead'
  | 'toggleAgentPanel'
  | 'stopAgent'
  | 'editorPianoRoll'
  | 'editorScore'
  | 'editorStep'
  | 'toolPointer'
  | 'toolPencil'
  | 'toolScissors'
  | 'toolGrid';

export const SNAP_DIVISIONS = [1, 2, 4, 8, 16, 32] as const;

const selectedClip = (s: Session) => (s.view.selectedClipId ? s.clips.find((c) => c.id === s.view.selectedClipId) ?? null : null);
const selectedTrack = (s: Session) => (s.view.selectedTrackId ? s.tracks.find((t) => t.id === s.view.selectedTrackId) ?? null : null);
const selectedNote = (s: Session) => {
  if (!s.view.selectedNoteId || !s.view.editorClipId) return null;
  const clip = s.clips.find((c) => c.id === s.view.editorClipId);
  if (!clip || clip.data.kind !== 'midi') return null;
  return clip.data.notes.find((n) => n.id === s.view.selectedNoteId) ?? null;
};

function define(defs: ActionDef[]): Record<ActionId, ActionDef> {
  const out = {} as Record<ActionId, ActionDef>;
  for (const d of defs) out[d.id] = d;
  return out;
}

export const actions = define([
  {
    id: 'undo',
    label: 'Undo',
    shortcut: { key: 'z', meta: true },
    enabled: (_s, store) => store.canUndo(),
    run: (store) => store.dispatch(commands.history.undo({})),
  },
  {
    id: 'redo',
    label: 'Redo',
    shortcut: { key: 'z', meta: true, shift: true },
    enabled: (_s, store) => store.canRedo(),
    run: (store) => store.dispatch(commands.history.redo({})),
  },
  { id: 'togglePlay', label: 'Play / Stop', shortcut: { key: 'Space' }, run: (store) => store.dispatch(commands.transport.togglePlay({})) },
  { id: 'stop', label: 'Stop', shortcut: { key: '0' }, run: (store) => store.dispatch(commands.transport.stop({})) },
  {
    id: 'record',
    label: 'Record',
    shortcut: { key: 'r' },
    checked: (s) => s.transport.recording,
    run: (store) => store.dispatch(commands.transport.setRecording({ recording: !store.getState().transport.recording })),
  },
  {
    id: 'cycle',
    label: 'Cycle',
    shortcut: { key: 'c' },
    checked: (s) => s.transport.cycle,
    run: (store) => store.dispatch(commands.transport.setCycle({ enabled: !store.getState().transport.cycle })),
  },
  { id: 'returnToStart', label: 'Go to Beginning', shortcut: { key: 'Enter' }, run: (store) => store.dispatch(commands.transport.returnToStart({})) },
  { id: 'rewind', label: 'Rewind One Bar', shortcut: { key: ',' }, run: (store) => store.dispatch(commands.transport.nudge({ bars: -1 })) },
  { id: 'forward', label: 'Forward One Bar', shortcut: { key: '.' }, run: (store) => store.dispatch(commands.transport.nudge({ bars: 1 })) },
  {
    id: 'metronome',
    label: 'Metronome Click',
    shortcut: { key: 'k' },
    checked: (s) => s.transport.metronome,
    run: (store) => store.dispatch(commands.transport.setMetronome({ enabled: !store.getState().transport.metronome })),
  },
  {
    id: 'deleteSelection',
    label: 'Delete',
    shortcut: { key: 'Backspace' },
    enabled: (s) => selectedNote(s) !== null || selectedClip(s) !== null,
    run: (store) => {
      const s = store.getState();
      const note = selectedNote(s);
      if (note && s.view.editorClipId) {
        store.dispatch(commands.note.remove({ clipId: s.view.editorClipId, noteId: note.id }));
        return;
      }
      const clip = selectedClip(s);
      if (clip) store.dispatch(commands.clip.remove({ clipId: clip.id }));
    },
  },
  {
    id: 'duplicateClip',
    label: 'Duplicate Clip',
    shortcut: { key: 'd', meta: true },
    enabled: (s) => selectedClip(s) !== null,
    run: (store) => {
      const clip = selectedClip(store.getState());
      if (clip) store.dispatch(commands.clip.duplicate({ clipId: clip.id, newClipId: newId('clip') }));
    },
  },
  {
    id: 'splitAtPlayhead',
    label: 'Split Clip at Playhead',
    shortcut: { key: 't', meta: true },
    enabled: (s) => {
      const clip = selectedClip(s);
      if (!clip) return false;
      const bar = beatsToBars(s.transport.positionBeats, s.transport.timeSignature);
      return bar > clip.startBar && bar < clip.startBar + clip.lengthBars;
    },
    run: (store) => {
      const s = store.getState();
      const clip = selectedClip(s);
      if (!clip) return;
      const bar = snapBars(beatsToBars(s.transport.positionBeats, s.transport.timeSignature), s.transport.snapDivision, s.transport.timeSignature);
      if (bar <= clip.startBar || bar >= clip.startBar + clip.lengthBars) return;
      store.dispatch(commands.clip.split({ clipId: clip.id, atBar: bar, newClipId: newId('clip') }));
    },
  },
  {
    id: 'openInEditor',
    label: 'Open in Editor',
    shortcut: { key: 'e' },
    enabled: (s) => selectedClip(s)?.data.kind === 'midi',
    run: (store) => {
      const clip = selectedClip(store.getState());
      if (clip?.data.kind === 'midi') store.dispatch(commands.view.setEditorClip({ clipId: clip.id }));
    },
  },
  {
    id: 'addAudioTrack',
    label: 'New Audio Track',
    shortcut: { key: 'a', meta: true, alt: true },
    run: (store) => store.dispatch(commands.track.add({ trackId: newId('track'), kind: 'audio', ...insertIndex(store) })),
  },
  {
    id: 'addMidiTrack',
    label: 'New MIDI Track',
    shortcut: { key: 's', meta: true, alt: true },
    run: (store) => store.dispatch(commands.track.add({ trackId: newId('track'), kind: 'midi', ...insertIndex(store) })),
  },
  {
    id: 'removeSelectedTrack',
    label: 'Delete Track',
    shortcut: { key: 'Backspace', meta: true },
    enabled: (s) => selectedTrack(s) !== null,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track) store.dispatch(commands.track.remove({ trackId: track.id }));
    },
  },
  {
    id: 'muteSelectedTrack',
    label: 'Mute Track',
    shortcut: { key: 'm' },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.mute ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track) store.dispatch(commands.track.setMute({ trackId: track.id, muted: !track.mute }));
    },
  },
  {
    id: 'soloSelectedTrack',
    label: 'Solo Track',
    shortcut: { key: 's' },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.solo ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track) store.dispatch(commands.track.setSolo({ trackId: track.id, solo: !track.solo }));
    },
  },
  {
    id: 'armSelectedTrack',
    label: 'Record-Arm Track',
    shortcut: { key: 'a' },
    enabled: (s) => selectedTrack(s) !== null,
    checked: (s) => selectedTrack(s)?.armed ?? false,
    run: (store) => {
      const track = selectedTrack(store.getState());
      if (track) store.dispatch(commands.track.setArmed({ trackId: track.id, armed: !track.armed }));
    },
  },
  { id: 'zoomIn', label: 'Zoom In', shortcut: { key: '=', meta: true }, run: (store) => zoomAroundPlayhead(store, Math.SQRT2) },
  { id: 'zoomOut', label: 'Zoom Out', shortcut: { key: '-', meta: true }, run: (store) => zoomAroundPlayhead(store, Math.SQRT1_2) },
  {
    id: 'zoomToFit',
    label: 'Zoom to Fit Session',
    shortcut: { key: 'z' },
    run: (store) => {
      const s = store.getState();
      const end = Math.max(8, ...s.clips.map((c) => c.startBar + c.lengthBars)) + 1;
      const width = viewportWidth(store);
      store.dispatch(commands.view.setZoom({ pixelsPerBar: width / end }));
      store.dispatch(commands.view.scrollTo({ bar: 0 }));
    },
  },
  {
    id: 'followPlayhead',
    label: 'Follow Playhead',
    shortcut: { key: 'f' },
    checked: (s) => s.view.followPlayhead,
    run: (store) => store.dispatch(commands.view.setFollowPlayhead({ enabled: !store.getState().view.followPlayhead })),
  },
  {
    id: 'toggleAgentPanel',
    label: 'Agent Panel',
    shortcut: { key: 'j', meta: true },
    checked: (s) => s.view.agentPanelOpen,
    run: (store) => store.dispatch(commands.view.setAgentPanelOpen({ open: !store.getState().view.agentPanelOpen })),
  },
  {
    id: 'stopAgent',
    label: 'Stop Current Agent Action',
    enabled: (s) => s.agent.current !== null,
    run: (store) => store.dispatch(commands.agent.stopCurrent({})),
  },
  { id: 'editorPianoRoll', label: 'Piano Roll', checked: (s) => s.view.editorMode === 'pianoRoll', run: (store) => store.dispatch(commands.view.setEditorMode({ mode: 'pianoRoll' })) },
  { id: 'editorScore', label: 'Score', checked: (s) => s.view.editorMode === 'score', run: (store) => store.dispatch(commands.view.setEditorMode({ mode: 'score' })) },
  { id: 'editorStep', label: 'Step', checked: (s) => s.view.editorMode === 'step', run: (store) => store.dispatch(commands.view.setEditorMode({ mode: 'step' })) },
  { id: 'toolPointer', label: 'Pointer Tool', shortcut: { key: '1' }, checked: (s) => s.view.arrangeTool === 'pointer', run: (store) => store.dispatch(commands.view.setArrangeTool({ tool: 'pointer' })) },
  { id: 'toolPencil', label: 'Pencil Tool', shortcut: { key: '2' }, checked: (s) => s.view.arrangeTool === 'pencil', run: (store) => store.dispatch(commands.view.setArrangeTool({ tool: 'pencil' })) },
  { id: 'toolScissors', label: 'Scissors Tool', shortcut: { key: '3' }, checked: (s) => s.view.arrangeTool === 'scissors', run: (store) => store.dispatch(commands.view.setArrangeTool({ tool: 'scissors' })) },
  { id: 'toolGrid', label: 'Grid Tool', shortcut: { key: '4' }, checked: (s) => s.view.arrangeTool === 'grid', run: (store) => store.dispatch(commands.view.setArrangeTool({ tool: 'grid' })) },
]);

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
      onScreen ? { factor, anchorBar: bar, anchorPx: px } : { factor, anchorBar: s.view.scrollBars + width / 2 / s.view.pixelsPerBar, anchorPx: width / 2 },
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
  return snap ? snapBars(bar, s.transport.snapDivision, s.transport.timeSignature) : bar;
}

export function locateToBar(store: SessionStore, bar: number): void {
  const s = store.getState();
  store.dispatch(commands.transport.setPosition({ beats: Math.max(0, barsToBeats(bar, s.transport.timeSignature)) }));
}
