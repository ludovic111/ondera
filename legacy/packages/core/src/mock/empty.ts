import type { Session } from '../model/types';
import { TRACK_PALETTE } from '../model/palette';
import { defaultStrip } from '../model/strip';
import { createMockSession } from './session';

/** A fresh session: two empty tracks, nothing else. Browser content is shared with the demo. */
export function createEmptySession(name = 'Untitled.ondera'): Session {
  const demo = createMockSession();
  return {
    name,
    audio: { sampleRate: 48000, bitDepth: 24, bufferSize: 128 },
    tracks: [
      { id: 'inst-1', name: 'Inst 1', kind: 'midi', color: TRACK_PALETTE.keys, volume: 0.8, pan: 0, mute: false, solo: false, armed: false, agentActive: false },
      { id: 'audio-1', name: 'Audio 1', kind: 'audio', color: TRACK_PALETTE.vox, volume: 0.8, pan: 0, mute: false, solo: false, armed: false, agentActive: false },
    ],
    clips: [],
    sources: {},
    transport: {
      playing: false,
      recording: false,
      positionBeats: 0,
      tempo: 120,
      timeSignature: { numerator: 4, denominator: 4 },
      key: 'C maj',
      cycle: false,
      cycleStartBar: 0,
      cycleEndBar: 4,
      metronome: true,
      snapDivision: 16,
    },
    view: {
      pixelsPerBar: 48,
      scrollBars: 0,
      agentPanelOpen: false,
      selectedTrackId: 'inst-1',
      selectedClipId: null,
      editorClipId: null,
      selectedNoteId: null,
      editorMode: 'pianoRoll',
      browserTab: 'instruments',
      browserSelection: null,
      arrangeTool: 'pointer',
      followPlayhead: true,
    },
    agent: { status: 'idle', transport: 'no agent connected', current: null, log: [], draft: '' },
    browser: demo.browser,
    strips: { 'inst-1': defaultStrip('midi'), 'audio-1': defaultStrip('audio') },
    meters: { masterL: 0, masterR: 0, cpu: 0, channelL: 0, channelR: 0 },
  };
}
