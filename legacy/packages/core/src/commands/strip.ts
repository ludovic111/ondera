import { defineCommand } from './define';
import { p } from './schema';
import { defaultStrip } from '../model/strip';
import type { ChannelStrip, Session } from '../model/types';

export const INSERT_STATES = ['active', 'bypassed', 'empty'] as const;

function stripOf(state: Session, trackId: string, command: string): ChannelStrip {
  const track = state.tracks.find((t) => t.id === trackId);
  if (!track) throw new Error(`${command}: no track with id "${trackId}"`);
  return state.strips[trackId] ?? defaultStrip(track.kind);
}

function withStrip(state: Session, trackId: string, strip: ChannelStrip): Session {
  return { ...state, strips: { ...state.strips, [trackId]: strip } };
}

export const setSendLevel = defineCommand({
  name: 'strip.setSendLevel',
  description: 'Set a send level in dB (use -Infinity, or anything below -60, for off).',
  params: { trackId: p.string(), sendIndex: p.number({ min: 0 }), levelDb: p.number({ max: 6 }) },
  apply: (state, { trackId, sendIndex, levelDb }) => {
    const strip = stripOf(state, trackId, 'strip.setSendLevel');
    const send = strip.sends[sendIndex];
    if (!send) throw new Error(`strip.setSendLevel: track "${trackId}" has no send ${sendIndex}`);
    const level = levelDb <= -60 ? -Infinity : levelDb;
    const sends = strip.sends.map((s, i) => (i === sendIndex ? { ...s, levelDb: level } : s));
    return withStrip(state, trackId, { ...strip, sends });
  },
});

export const setInsertState = defineCommand({
  name: 'strip.setInsertState',
  description: 'Activate, bypass or clear an insert slot.',
  params: { trackId: p.string(), slotIndex: p.number({ min: 0 }), state: p.enum(INSERT_STATES) },
  apply: (session, { trackId, slotIndex, state }) => {
    const strip = stripOf(session, trackId, 'strip.setInsertState');
    const slot = strip.inserts[slotIndex];
    if (!slot) throw new Error(`strip.setInsertState: track "${trackId}" has no insert slot ${slotIndex}`);
    const inserts = strip.inserts.map((s, i) => {
      if (i !== slotIndex) return s;
      if (state === 'empty') return { name: 'Empty slot', meta: '', state };
      return { ...s, state, meta: state === 'bypassed' ? 'bypassed' : s.meta === 'bypassed' ? '' : s.meta };
    });
    return withStrip(session, trackId, { ...strip, inserts });
  },
});

export const setInstrument = defineCommand({
  name: 'strip.setInstrument',
  description: 'Load an instrument preset on a MIDI track.',
  params: { trackId: p.string(), instrument: p.string() },
  apply: (state, { trackId, instrument }) => {
    const track = state.tracks.find((t) => t.id === trackId);
    if (!track) throw new Error(`strip.setInstrument: no track with id "${trackId}"`);
    if (track.kind !== 'midi') throw new Error(`strip.setInstrument: track "${trackId}" is not a MIDI track`);
    return withStrip(state, trackId, { ...stripOf(state, trackId, 'strip.setInstrument'), instrument });
  },
});

export const setInsert = defineCommand({
  name: 'strip.setInsert',
  description: 'Put an effect in an insert slot (active).',
  params: { trackId: p.string(), slotIndex: p.number({ min: 0 }), name: p.string() },
  apply: (state, { trackId, slotIndex, name }) => {
    const strip = stripOf(state, trackId, 'strip.setInsert');
    if (!strip.inserts[slotIndex]) throw new Error(`strip.setInsert: track "${trackId}" has no insert slot ${slotIndex}`);
    const inserts = strip.inserts.map((s, i) => (i === slotIndex ? { name, meta: '', state: 'active' as const } : s));
    return withStrip(state, trackId, { ...strip, inserts });
  },
});
