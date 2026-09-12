import { defineCommand } from './define';
import { p } from './schema';
import type { Session, Track, TrackId } from '../model/types';
import { defaultStrip } from '../model/strip';
import { TRACK_PALETTE } from '../model/palette';

function updateTrack(state: Session, trackId: TrackId, patch: Partial<Track>): Session {
  let found = false;
  const tracks = state.tracks.map((t) => {
    if (t.id !== trackId) return t;
    found = true;
    return { ...t, ...patch };
  });
  if (!found) throw new Error(`track.update: no track with id "${trackId}"`);
  return { ...state, tracks };
}

export const setMute = defineCommand({
  name: 'track.setMute',
  description: 'Mute or unmute a track.',
  params: { trackId: p.string(), muted: p.boolean() },
  apply: (state, { trackId, muted }) => updateTrack(state, trackId, { mute: muted }),
});

export const setSolo = defineCommand({
  name: 'track.setSolo',
  description: 'Solo or unsolo a track.',
  params: { trackId: p.string(), solo: p.boolean() },
  apply: (state, { trackId, solo }) => updateTrack(state, trackId, { solo }),
});

export const setArmed = defineCommand({
  name: 'track.setArmed',
  description: 'Arm or disarm a track for recording.',
  params: { trackId: p.string(), armed: p.boolean() },
  apply: (state, { trackId, armed }) => updateTrack(state, trackId, { armed }),
});

export const setVolume = defineCommand({
  name: 'track.setVolume',
  description: 'Set a track fader, 0..1 linear.',
  params: { trackId: p.string(), volume: p.number({ min: 0, max: 1 }) },
  apply: (state, { trackId, volume }) => updateTrack(state, trackId, { volume }),
});

export const setPan = defineCommand({
  name: 'track.setPan',
  description: 'Set a track pan, -100 (left) .. 100 (right).',
  params: { trackId: p.string(), pan: p.number({ min: -100, max: 100 }) },
  apply: (state, { trackId, pan }) => updateTrack(state, trackId, { pan }),
});

export const rename = defineCommand({
  name: 'track.rename',
  description: 'Rename a track.',
  params: { trackId: p.string(), name: p.string() },
  apply: (state, { trackId, name }) => updateTrack(state, trackId, { name }),
});

export const select = defineCommand({
  name: 'track.select',
  description: 'Select a track; the inspector follows the selection.',
  transient: true,
  params: { trackId: p.string() },
  apply: (state, { trackId }) => {
    if (!state.tracks.some((t) => t.id === trackId)) {
      throw new Error(`track.select: no track with id "${trackId}"`);
    }
    return { ...state, view: { ...state.view, selectedTrackId: trackId } };
  },
});

export const setColor = defineCommand({
  name: 'track.setColor',
  description: 'Set a track colour (any CSS colour string, normally one from the palette).',
  params: { trackId: p.string(), color: p.string() },
  apply: (state, { trackId, color }) => updateTrack(state, trackId, { color }),
});

export const add = defineCommand({
  name: 'track.add',
  description: 'Create a track. Inserted at `index` (default: the end) and selected.',
  params: {
    trackId: p.string('Id for the new track'),
    kind: p.enum(['audio', 'midi']),
    name: p.optional(p.string()),
    color: p.optional(p.string()),
    index: p.optional(p.number({ min: 0 })),
  },
  apply: (state, { trackId, kind, name, color, index }) => {
    if (state.tracks.some((t) => t.id === trackId)) throw new Error(`track.add: track id "${trackId}" already exists`);
    const n = state.tracks.length + 1;
    const palette = Object.values(TRACK_PALETTE);
    const track: Track = {
      id: trackId,
      name: name ?? (kind === 'audio' ? `Audio ${n}` : `Inst ${n}`),
      kind,
      color: color ?? palette[(n - 1) % palette.length]!,
      volume: 0.75,
      pan: 0,
      mute: false,
      solo: false,
      armed: false,
      agentActive: false,
    };
    const at = Math.min(index ?? state.tracks.length, state.tracks.length);
    const tracks = [...state.tracks.slice(0, at), track, ...state.tracks.slice(at)];
    return {
      ...state,
      tracks,
      strips: { ...state.strips, [trackId]: defaultStrip(kind) },
      view: { ...state.view, selectedTrackId: trackId },
    };
  },
});

export const remove = defineCommand({
  name: 'track.remove',
  description: 'Delete a track and every clip on it.',
  params: { trackId: p.string() },
  apply: (state, { trackId }) => {
    const index = state.tracks.findIndex((t) => t.id === trackId);
    if (index < 0) throw new Error(`track.remove: no track with id "${trackId}"`);
    const tracks = state.tracks.filter((t) => t.id !== trackId);
    const removedClips = new Set(state.clips.filter((c) => c.trackId === trackId).map((c) => c.id));
    const clips = state.clips.filter((c) => !removedClips.has(c.id));
    const strips = { ...state.strips };
    delete strips[trackId];
    const view = { ...state.view };
    if (view.selectedTrackId === trackId) view.selectedTrackId = tracks[Math.min(index, tracks.length - 1)]?.id ?? null;
    if (view.selectedClipId && removedClips.has(view.selectedClipId)) view.selectedClipId = null;
    if (view.editorClipId && removedClips.has(view.editorClipId)) {
      view.editorClipId = null;
      view.selectedNoteId = null;
    }
    return { ...state, tracks, clips, strips, view };
  },
});

export const move = defineCommand({
  name: 'track.move',
  description: 'Reorder a track to a new index.',
  params: { trackId: p.string(), index: p.number({ min: 0 }) },
  apply: (state, { trackId, index }) => {
    const from = state.tracks.findIndex((t) => t.id === trackId);
    if (from < 0) throw new Error(`track.move: no track with id "${trackId}"`);
    const to = Math.min(index, state.tracks.length - 1);
    if (to === from) return state;
    const tracks = [...state.tracks];
    const [track] = tracks.splice(from, 1);
    tracks.splice(to, 0, track!);
    return { ...state, tracks };
  },
});
