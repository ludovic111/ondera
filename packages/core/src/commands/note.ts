import { defineCommand } from './define';
import { p } from './schema';
import type { Clip, MidiClipData, Note, Session } from '../model/types';

function findMidiClip(state: Session, clipId: string, command: string): Clip & { data: MidiClipData } {
  const clip = state.clips.find((c) => c.id === clipId);
  if (!clip) throw new Error(`${command}: no clip with id "${clipId}"`);
  if (clip.data.kind !== 'midi') throw new Error(`${command}: clip "${clipId}" is not a MIDI clip`);
  return clip as Clip & { data: MidiClipData };
}

function withNotes(state: Session, clip: Clip, notes: Note[]): Session {
  const next: Clip = { ...clip, data: { kind: 'midi', notes } };
  return { ...state, clips: state.clips.map((c) => (c.id === clip.id ? next : c)) };
}

export const add = defineCommand({
  name: 'note.add',
  description: 'Add a note to a MIDI clip. Start and length are in beats relative to the clip start.',
  params: {
    clipId: p.string(),
    noteId: p.string('Id for the new note'),
    start: p.number({ min: 0 }),
    length: p.number({ min: 0.0625 }),
    pitch: p.number({ min: 0, max: 127 }),
    velocity: p.optional(p.number({ min: 1, max: 127 })),
  },
  apply: (state, { clipId, noteId, start, length, pitch, velocity }) => {
    const clip = findMidiClip(state, clipId, 'note.add');
    if (clip.data.notes.some((n) => n.id === noteId)) throw new Error(`note.add: note id "${noteId}" already exists`);
    const note: Note = { id: noteId, start, length, pitch, velocity: velocity ?? 96 };
    const next = withNotes(state, clip, [...clip.data.notes, note]);
    return { ...next, view: { ...next.view, selectedNoteId: noteId } };
  },
});

export const update = defineCommand({
  name: 'note.update',
  description: 'Change a note. Any of start, length, pitch and velocity may be given.',
  params: {
    clipId: p.string(),
    noteId: p.string(),
    start: p.optional(p.number({ min: 0 })),
    length: p.optional(p.number({ min: 0.0625 })),
    pitch: p.optional(p.number({ min: 0, max: 127 })),
    velocity: p.optional(p.number({ min: 1, max: 127 })),
  },
  apply: (state, { clipId, noteId, start, length, pitch, velocity }) => {
    const clip = findMidiClip(state, clipId, 'note.update');
    let found = false;
    const notes = clip.data.notes.map((n) => {
      if (n.id !== noteId) return n;
      found = true;
      return {
        ...n,
        start: start ?? n.start,
        length: length ?? n.length,
        pitch: pitch ?? n.pitch,
        velocity: velocity ?? n.velocity,
      };
    });
    if (!found) throw new Error(`note.update: no note "${noteId}" in clip "${clipId}"`);
    return withNotes(state, clip, notes);
  },
});

export const remove = defineCommand({
  name: 'note.remove',
  description: 'Delete a note from a MIDI clip.',
  params: { clipId: p.string(), noteId: p.string() },
  apply: (state, { clipId, noteId }) => {
    const clip = findMidiClip(state, clipId, 'note.remove');
    if (!clip.data.notes.some((n) => n.id === noteId)) throw new Error(`note.remove: no note "${noteId}" in clip "${clipId}"`);
    const next = withNotes(state, clip, clip.data.notes.filter((n) => n.id !== noteId));
    return {
      ...next,
      view: { ...next.view, selectedNoteId: next.view.selectedNoteId === noteId ? null : next.view.selectedNoteId },
    };
  },
});

export const select = defineCommand({
  name: 'note.select',
  description: 'Select a note in the editor, or clear the note selection when no id is given.',
  transient: true,
  params: { noteId: p.optional(p.string()) },
  apply: (state, { noteId }) => ({ ...state, view: { ...state.view, selectedNoteId: noteId ?? null } }),
});
