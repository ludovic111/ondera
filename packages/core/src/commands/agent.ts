import { defineCommand } from './define';
import { p } from './schema';

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
