import { defineCommand } from './define';
import { p } from './schema';

export const select = defineCommand({
  name: 'clip.select',
  description: 'Select a clip. Also selects its track and opens it in the editor.',
  params: { clipId: p.string() },
  apply: (state, { clipId }) => {
    const clip = state.clips.find((c) => c.id === clipId);
    if (!clip) throw new Error(`clip.select: no clip with id "${clipId}"`);
    return {
      ...state,
      view: {
        ...state.view,
        selectedClipId: clipId,
        selectedTrackId: clip.trackId,
        editorClipId: clip.data.kind === 'midi' ? clipId : state.view.editorClipId,
      },
    };
  },
});

export const clearSelection = defineCommand({
  name: 'clip.clearSelection',
  description: 'Deselect any selected clip.',
  params: {},
  apply: (state) => ({ ...state, view: { ...state.view, selectedClipId: null } }),
});

export const rename = defineCommand({
  name: 'clip.rename',
  description: 'Rename a clip.',
  params: { clipId: p.string(), name: p.string() },
  apply: (state, { clipId, name }) => {
    let found = false;
    const clips = state.clips.map((c) => {
      if (c.id !== clipId) return c;
      found = true;
      return { ...c, name };
    });
    if (!found) throw new Error(`clip.rename: no clip with id "${clipId}"`);
    return { ...state, clips };
  },
});
