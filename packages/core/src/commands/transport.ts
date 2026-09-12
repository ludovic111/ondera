import { defineCommand } from './define';
import { p } from './schema';
import { barsToBeats } from '../model/time';
import type { Session, Transport } from '../model/types';

const setTransport = (state: Session, patch: Partial<Transport>): Session => ({
  ...state,
  transport: { ...state.transport, ...patch },
});

export const play = defineCommand({
  name: 'transport.play',
  description: 'Start playback from the current position.',
  params: {},
  apply: (state) => setTransport(state, { playing: true }),
});

export const stop = defineCommand({
  name: 'transport.stop',
  description: 'Stop playback. A second stop returns to the cycle start or bar 1.',
  params: {},
  apply: (state) => {
    if (state.transport.playing) return setTransport(state, { playing: false, recording: false });
    const home = state.transport.cycle
      ? barsToBeats(state.transport.cycleStartBar, state.transport.timeSignature)
      : 0;
    return setTransport(state, { positionBeats: home });
  },
});

export const togglePlay = defineCommand({
  name: 'transport.togglePlay',
  description: 'Toggle between play and stop (space bar).',
  params: {},
  apply: (state) =>
    state.transport.playing ? stop.def.apply(state, {}) : play.def.apply(state, {}),
});

export const returnToStart = defineCommand({
  name: 'transport.returnToStart',
  description: 'Move the playhead to bar 1.',
  params: {},
  apply: (state) => setTransport(state, { positionBeats: 0 }),
});

export const nudge = defineCommand({
  name: 'transport.nudge',
  description: 'Move the playhead by a number of bars (negative to rewind).',
  params: { bars: p.number({ description: 'Bars to move, may be negative' }) },
  apply: (state, { bars }) =>
    setTransport(state, {
      positionBeats: Math.max(
        0,
        state.transport.positionBeats + barsToBeats(bars, state.transport.timeSignature),
      ),
    }),
});

export const setPosition = defineCommand({
  name: 'transport.setPosition',
  description: 'Set the playhead position in beats.',
  params: { beats: p.number({ min: 0 }) },
  apply: (state, { beats }) => setTransport(state, { positionBeats: beats }),
});

/** Engine-driven playhead advance. Transient: not recorded in history. */
export const tick = defineCommand({
  name: 'transport.tick',
  description: 'Advance the playhead (sent by the engine while playing).',
  params: { beats: p.number({ min: 0 }) },
  transient: true,
  apply: (state, { beats }) => {
    const t = state.transport;
    if (!t.playing) return state;
    let next = beats;
    if (t.cycle) {
      const start = barsToBeats(t.cycleStartBar, t.timeSignature);
      const end = barsToBeats(t.cycleEndBar, t.timeSignature);
      if (next >= end && end > start) next = start + ((next - start) % (end - start));
    }
    return setTransport(state, { positionBeats: next });
  },
});

export const setRecording = defineCommand({
  name: 'transport.setRecording',
  description: 'Arm or disarm the transport record button.',
  params: { recording: p.boolean() },
  apply: (state, { recording }) => setTransport(state, { recording }),
});

export const setCycle = defineCommand({
  name: 'transport.setCycle',
  description: 'Enable or disable cycle (loop) playback.',
  params: { enabled: p.boolean() },
  apply: (state, { enabled }) => setTransport(state, { cycle: enabled }),
});

export const setCycleRange = defineCommand({
  name: 'transport.setCycleRange',
  description: 'Set the cycle range in bars (0-based, end exclusive).',
  params: { startBar: p.number({ min: 0 }), endBar: p.number({ min: 0 }) },
  apply: (state, { startBar, endBar }) =>
    setTransport(state, { cycleStartBar: startBar, cycleEndBar: Math.max(endBar, startBar + 1) }),
});

export const setTempo = defineCommand({
  name: 'transport.setTempo',
  description: 'Set the project tempo in BPM.',
  params: { bpm: p.number({ min: 20, max: 400 }) },
  apply: (state, { bpm }) => setTransport(state, { tempo: bpm }),
});

export const setMetronome = defineCommand({
  name: 'transport.setMetronome',
  description: 'Turn the click on or off.',
  params: { enabled: p.boolean() },
  apply: (state, { enabled }) => setTransport(state, { metronome: enabled }),
});

export const setSnap = defineCommand({
  name: 'transport.setSnap',
  description: 'Set the snap grid as a note division (4 = quarter, 16 = sixteenth).',
  params: { division: p.number({ min: 1, max: 128 }) },
  apply: (state, { division }) => setTransport(state, { snapDivision: division }),
});
