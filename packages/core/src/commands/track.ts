import { defineCommand } from './define';
import { p } from './schema';
import type { Session, Track, TrackId } from '../model/types';

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
  params: { trackId: p.string() },
  apply: (state, { trackId }) => {
    if (!state.tracks.some((t) => t.id === trackId)) {
      throw new Error(`track.select: no track with id "${trackId}"`);
    }
    return { ...state, view: { ...state.view, selectedTrackId: trackId } };
  },
});
