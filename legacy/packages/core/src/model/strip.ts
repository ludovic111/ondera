import type { ChannelStrip, TrackKind } from './types';

/** The channel strip a fresh track gets. Also what the inspector shows for tracks without one. */
export function defaultStrip(kind: TrackKind): ChannelStrip {
  return {
    instrument: kind === 'midi' ? 'Ondera Synth' : '—',
    input: kind === 'midi' ? 'All MIDI' : 'Input 1',
    output: 'Stereo Out',
    inserts: [
      { name: 'Empty slot', meta: '', state: 'empty' },
      { name: 'Empty slot', meta: '', state: 'empty' },
      { name: 'Empty slot', meta: '', state: 'empty' },
      { name: 'Empty slot', meta: '', state: 'empty' },
    ],
    sends: [
      { name: 'A · Reverb', levelDb: -Infinity },
      { name: 'B · Delay', levelDb: -Infinity },
    ],
  };
}
