import { defineCommand } from './define';
import { p } from './schema';

/** Log entries not tied to a track use the neutral dot colour. */
export const NEUTRAL_LOG_COLOR = '#7f7e7a';

export const setDraft = defineCommand({
  name: 'agent.setDraft',
  description: 'Update the text in the agent prompt box.',
  params: { text: p.string() },
  transient: true,
  apply: (state, { text }) => ({ ...state, agent: { ...state.agent, draft: text } }),
});

export const toggleRevert = defineCommand({
  name: 'agent.toggleRevert',
  description: 'Revert (or redo) a change the agent made this session.',
  params: { entryId: p.string() },
  apply: (state, { entryId }) => {
    let found = false;
    const log = state.agent.log.map((e) => {
      if (e.id !== entryId) return e;
      found = true;
      return { ...e, reverted: !e.reverted };
    });
    if (!found) throw new Error(`agent.toggleRevert: no log entry "${entryId}"`);
    return { ...state, agent: { ...state.agent, log } };
  },
});

export const stopCurrent = defineCommand({
  name: 'agent.stopCurrent',
  description: 'Interrupt the action the agent is currently performing.',
  params: {},
  apply: (state) => ({
    ...state,
    agent: { ...state.agent, status: 'idle', current: null },
    tracks: state.tracks.map((t) => ({ ...t, agentActive: false })),
    clips: state.clips.map((c) => ({ ...c, agent: false })),
  }),
});

export const submit = defineCommand({
  name: 'agent.submit',
  description: 'Send the prompt in the agent box. Phase 1 has no agent connected: the request is logged and the box cleared.',
  params: { entryId: p.string('Id for the new log entry'), text: p.string() },
  apply: (state, { entryId, text }) => {
    const trimmed = text.trim();
    if (!trimmed) return state;
    if (state.agent.log.some((e) => e.id === entryId)) throw new Error(`agent.submit: log entry "${entryId}" already exists`);
    const entry = {
      id: entryId,
      title: trimmed,
      detail: 'queued · no agent connected in Phase 1',
      color: NEUTRAL_LOG_COLOR,
      live: false,
      reverted: false,
    };
    return { ...state, agent: { ...state.agent, draft: '', log: [entry, ...state.agent.log] } };
  },
});
