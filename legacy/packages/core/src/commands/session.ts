import { defineCommand } from './define';
import { p } from './schema';

export const rename = defineCommand({
  name: 'session.rename',
  description: 'Rename the session.',
  params: { name: p.string() },
  apply: (state, { name }) => ({ ...state, name }),
});

export const addSource = defineCommand({
  name: 'session.addSource',
  description: 'Register an audio source (an imported file or a recording) so clips can play it.',
  params: {
    sourceId: p.string(),
    name: p.string(),
    durationSeconds: p.number({ min: 0 }),
    sampleRate: p.number({ min: 1 }),
    channels: p.number({ min: 1, max: 2 }),
    origin: p.enum(['file', 'recording']),
    fileName: p.string('File name inside the session bundle'),
  },
  apply: (state, { sourceId, name, durationSeconds, sampleRate, channels, origin, fileName }) => {
    if (state.sources[sourceId]) throw new Error(`session.addSource: source "${sourceId}" already exists`);
    return {
      ...state,
      sources: { ...state.sources, [sourceId]: { id: sourceId, name, durationSeconds, sampleRate, channels, origin, fileName } },
    };
  },
});

/**
 * Replaces the whole session (open / new). A meta-command like undo: the
 * store swaps its state for the session the host hands it out of band and
 * clears the undo stack.
 */
export const load = defineCommand({
  name: 'session.load',
  description: 'Replace the session with one loaded by the host. Clears undo history.',
  params: { token: p.string('Handle to a session the host has staged with store.stage()') },
  apply: (state) => state,
});

/** Live meter readings from the engine. Transient. */
export const updateMeters = defineCommand({
  name: 'session.updateMeters',
  description: 'Engine meter readings, 0..1 per side.',
  params: {
    masterL: p.number({ min: 0, max: 1 }),
    masterR: p.number({ min: 0, max: 1 }),
    channelL: p.number({ min: 0, max: 1 }),
    channelR: p.number({ min: 0, max: 1 }),
    cpu: p.number({ min: 0, max: 1 }),
  },
  transient: true,
  apply: (state, meters) => ({ ...state, meters }),
});
