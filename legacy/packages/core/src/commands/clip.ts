import { defineCommand } from './define';
import { p } from './schema';
import { barsToBeats, barsToSeconds } from '../model/time';
import type { Clip, ClipId, Note, Session } from '../model/types';

function findClip(state: Session, clipId: ClipId, command: string): Clip {
  const clip = state.clips.find((c) => c.id === clipId);
  if (!clip) throw new Error(`${command}: no clip with id "${clipId}"`);
  return clip;
}

function replaceClip(state: Session, next: Clip): Session {
  return { ...state, clips: state.clips.map((c) => (c.id === next.id ? next : c)) };
}

/** Drop view references to a clip that no longer exists. */
function forgetClip(state: Session, clipId: ClipId): Session {
  const view = state.view;
  return {
    ...state,
    view: {
      ...view,
      selectedClipId: view.selectedClipId === clipId ? null : view.selectedClipId,
      editorClipId: view.editorClipId === clipId ? null : view.editorClipId,
      selectedNoteId: view.editorClipId === clipId ? null : view.selectedNoteId,
    },
  };
}

export const select = defineCommand({
  name: 'clip.select',
  description: 'Select a clip. Also selects its track and opens it in the editor.',
  transient: true,
  params: { clipId: p.string() },
  apply: (state, { clipId }) => {
    const clip = findClip(state, clipId, 'clip.select');
    const editorChanges = clip.data.kind === 'midi' && state.view.editorClipId !== clipId;
    return {
      ...state,
      view: {
        ...state.view,
        selectedClipId: clipId,
        selectedTrackId: clip.trackId,
        editorClipId: clip.data.kind === 'midi' ? clipId : state.view.editorClipId,
        selectedNoteId: editorChanges ? null : state.view.selectedNoteId,
      },
    };
  },
});

export const clearSelection = defineCommand({
  name: 'clip.clearSelection',
  description: 'Deselect any selected clip.',
  transient: true,
  params: {},
  apply: (state) => ({ ...state, view: { ...state.view, selectedClipId: null } }),
});

export const rename = defineCommand({
  name: 'clip.rename',
  description: 'Rename a clip.',
  params: { clipId: p.string(), name: p.string() },
  apply: (state, { clipId, name }) => replaceClip(state, { ...findClip(state, clipId, 'clip.rename'), name }),
});

export const create = defineCommand({
  name: 'clip.create',
  description: 'Create a clip on a track: an empty MIDI clip on a MIDI track, or an audio clip playing `sourceId` on an audio track.',
  params: {
    clipId: p.string('Id for the new clip'),
    trackId: p.string(),
    startBar: p.number({ min: 0 }),
    lengthBars: p.number({ min: 0.0625 }),
    name: p.optional(p.string()),
    sourceId: p.optional(p.string('Audio source to play; required on audio tracks')),
    offsetSeconds: p.optional(p.number({ min: 0 })),
  },
  apply: (state, { clipId, trackId, startBar, lengthBars, name, sourceId, offsetSeconds }) => {
    const track = state.tracks.find((t) => t.id === trackId);
    if (!track) throw new Error(`clip.create: no track with id "${trackId}"`);
    if (state.clips.some((c) => c.id === clipId)) throw new Error(`clip.create: clip id "${clipId}" already exists`);
    if (track.kind === 'audio') {
      if (!sourceId) throw new Error('clip.create: audio clips need a sourceId');
      if (!state.sources[sourceId]) throw new Error(`clip.create: no audio source "${sourceId}"`);
    }
    const count = state.clips.filter((c) => c.trackId === trackId).length + 1;
    const clip: Clip = {
      id: clipId,
      trackId,
      name: name ?? (track.kind === 'audio' ? state.sources[sourceId!]!.name : `${track.name} ${count}`),
      startBar,
      lengthBars,
      agent: false,
      data: track.kind === 'midi' ? { kind: 'midi', notes: [] } : { kind: 'audio', sourceId: sourceId!, offsetSeconds: offsetSeconds ?? 0 },
    };
    return {
      ...state,
      clips: [...state.clips, clip],
      view: {
        ...state.view,
        selectedClipId: clipId,
        selectedTrackId: trackId,
        editorClipId: clip.data.kind === 'midi' ? clipId : state.view.editorClipId,
        selectedNoteId: clip.data.kind === 'midi' ? null : state.view.selectedNoteId,
      },
    };
  },
});

export const move = defineCommand({
  name: 'clip.move',
  description: 'Move a clip to a new start bar, optionally onto another track of the same kind.',
  params: {
    clipId: p.string(),
    startBar: p.number({ min: 0 }),
    trackId: p.optional(p.string('Target track; defaults to the current one')),
  },
  apply: (state, { clipId, startBar, trackId }) => {
    const clip = findClip(state, clipId, 'clip.move');
    let target = clip.trackId;
    if (trackId !== undefined && trackId !== clip.trackId) {
      const track = state.tracks.find((t) => t.id === trackId);
      if (!track) throw new Error(`clip.move: no track with id "${trackId}"`);
      if (track.kind !== clip.data.kind) throw new Error(`clip.move: cannot put a ${clip.data.kind} clip on a ${track.kind} track`);
      target = trackId;
    }
    const next = replaceClip(state, { ...clip, startBar, trackId: target });
    return state.view.selectedClipId === clipId
      ? { ...next, view: { ...next.view, selectedTrackId: target } }
      : next;
  },
});

/** Shift clip-relative notes by `deltaBeats` and drop the ones that fall outside the clip. */
function trimNotes(notes: readonly Note[], deltaBeats: number, lengthBeats: number): Note[] {
  const out: Note[] = [];
  for (const n of notes) {
    const start = n.start + deltaBeats;
    if (start < 0 || start >= lengthBeats) continue;
    out.push({ ...n, start, length: Math.min(n.length, lengthBeats - start) });
  }
  return out;
}

export const resize = defineCommand({
  name: 'clip.resize',
  description: 'Trim a clip by setting its start and length in bars. Notes outside the new range are dropped.',
  params: { clipId: p.string(), startBar: p.number({ min: 0 }), lengthBars: p.number({ min: 0.0625 }) },
  apply: (state, { clipId, startBar, lengthBars }) => {
    const clip = findClip(state, clipId, 'clip.resize');
    const sig = state.transport.timeSignature;
    let data = clip.data;
    if (data.kind === 'midi') {
      data = { kind: 'midi', notes: trimNotes(data.notes, barsToBeats(clip.startBar - startBar, sig), barsToBeats(lengthBars, sig)) };
    } else {
      // Trimming the front of an audio clip moves its read offset so the audio stays put.
      const deltaSeconds = barsToSeconds(startBar - clip.startBar, state.transport.tempo, sig);
      data = { ...data, offsetSeconds: Math.max(0, data.offsetSeconds + deltaSeconds) };
    }
    return replaceClip(state, { ...clip, startBar, lengthBars, data });
  },
});

export const split = defineCommand({
  name: 'clip.split',
  description: 'Cut a clip in two at a bar position. The right half gets the new id.',
  params: { clipId: p.string(), atBar: p.number({ min: 0 }), newClipId: p.string('Id for the right half') },
  apply: (state, { clipId, atBar, newClipId }) => {
    const clip = findClip(state, clipId, 'clip.split');
    if (atBar <= clip.startBar || atBar >= clip.startBar + clip.lengthBars) {
      throw new Error(`clip.split: bar ${atBar} is not inside clip "${clipId}"`);
    }
    if (state.clips.some((c) => c.id === newClipId)) throw new Error(`clip.split: clip id "${newClipId}" already exists`);
    const sig = state.transport.timeSignature;
    const leftLen = atBar - clip.startBar;
    const rightLen = clip.startBar + clip.lengthBars - atBar;
    const cutBeats = barsToBeats(leftLen, sig);
    let leftData = clip.data;
    let rightData = clip.data;
    if (clip.data.kind === 'midi') {
      leftData = { kind: 'midi', notes: trimNotes(clip.data.notes, 0, cutBeats) };
      rightData = {
        kind: 'midi',
        notes: trimNotes(clip.data.notes, -cutBeats, barsToBeats(rightLen, sig)).map((n) => ({ ...n, id: `${n.id}:r` })),
      };
    } else {
      rightData = { ...clip.data, offsetSeconds: clip.data.offsetSeconds + barsToSeconds(leftLen, state.transport.tempo, sig) };
    }
    const left: Clip = { ...clip, lengthBars: leftLen, data: leftData };
    const right: Clip = { ...clip, id: newClipId, startBar: atBar, lengthBars: rightLen, data: rightData, agent: false };
    const clips: Clip[] = [];
    for (const c of state.clips) {
      if (c.id === clipId) clips.push(left, right);
      else clips.push(c);
    }
    return { ...state, clips };
  },
});

export const duplicate = defineCommand({
  name: 'clip.duplicate',
  description: 'Copy a clip and place the copy right after it on the same track.',
  params: { clipId: p.string(), newClipId: p.string('Id for the copy') },
  apply: (state, { clipId, newClipId }) => {
    const clip = findClip(state, clipId, 'clip.duplicate');
    if (state.clips.some((c) => c.id === newClipId)) throw new Error(`clip.duplicate: clip id "${newClipId}" already exists`);
    const data =
      clip.data.kind === 'midi'
        ? { kind: 'midi' as const, notes: clip.data.notes.map((n) => ({ ...n, id: `${n.id}:d`, agent: false })) }
        : clip.data;
    const copy: Clip = { ...clip, id: newClipId, startBar: clip.startBar + clip.lengthBars, data, agent: false };
    return {
      ...state,
      clips: [...state.clips, copy],
      view: {
        ...state.view,
        selectedClipId: newClipId,
        editorClipId: copy.data.kind === 'midi' ? newClipId : state.view.editorClipId,
        selectedNoteId: null,
      },
    };
  },
});

export const remove = defineCommand({
  name: 'clip.remove',
  description: 'Delete a clip.',
  params: { clipId: p.string() },
  apply: (state, { clipId }) => {
    findClip(state, clipId, 'clip.remove');
    return forgetClip({ ...state, clips: state.clips.filter((c) => c.id !== clipId) }, clipId);
  },
});
