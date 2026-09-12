/** Offline render of the whole session (or a bar range) to a stereo AudioBuffer. */
import { barsToBeats, barsToSeconds, type Session } from '@ondera/core';
import { Graph } from './graph';
import { library } from './library';
import { encodeWav } from './wav';

export function sessionEndBar(state: Session): number {
  return Math.max(1, ...state.clips.map((c) => c.startBar + c.lengthBars));
}

export async function bounce(state: Session, fromBar = 0, toBar = sessionEndBar(state), tailSeconds = 2): Promise<ArrayBuffer> {
  const { tempo, timeSignature } = state.transport;
  const sampleRate = 48000;
  const seconds = barsToSeconds(toBar - fromBar, tempo, timeSignature) + tailSeconds;
  const ctx = new OfflineAudioContext(2, Math.ceil(seconds * sampleRate), sampleRate);
  // Generated sources were built for the realtime context's sample rate; reuse the same buffers.
  const graph = new Graph(ctx);
  graph.sync({ ...state, transport: { ...state.transport, metronome: false } });
  // Sources that are still generating would be missing; wait a tick for the library.
  await new Promise((r) => setTimeout(r, 30));
  if (Object.values(state.sources).some((s) => !library.has(s.id))) await new Promise((r) => setTimeout(r, 300));
  graph.scheduleRange({
    fromBeats: barsToBeats(fromBar, timeSignature),
    toBeats: barsToBeats(toBar, timeSignature),
    atTime: 0,
    hardEndBeats: barsToBeats(toBar, timeSignature),
    startMidClips: true,
  });
  const rendered = await ctx.startRendering();
  return encodeWav(rendered);
}
