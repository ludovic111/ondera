import { defineCommand } from './define';
import { p } from './schema';
import type { Session, View } from '../model/types';

export const ZOOM_MIN_PX_PER_BAR = 12;
export const ZOOM_MAX_PX_PER_BAR = 480;

const setView = (state: Session, patch: Partial<View>): Session => ({
  ...state,
  view: { ...state.view, ...patch },
});

const clampZoom = (px: number) => Math.min(ZOOM_MAX_PX_PER_BAR, Math.max(ZOOM_MIN_PX_PER_BAR, px));

export const setZoom = defineCommand({
  name: 'view.setZoom',
  description: 'Set arrangement zoom in pixels per bar. Optionally keep a bar position fixed on screen.',
  params: {
    pixelsPerBar: p.number({ min: ZOOM_MIN_PX_PER_BAR, max: ZOOM_MAX_PX_PER_BAR }),
    anchorBar: p.optional(p.number({ description: 'Bar under the cursor to keep stationary' })),
    anchorPx: p.optional(p.number({ description: 'Screen x of the anchor bar, in lane pixels' })),
  },
  apply: (state, { pixelsPerBar, anchorBar, anchorPx }) => {
    const next = clampZoom(pixelsPerBar);
    let scrollBars = state.view.scrollBars;
    if (anchorBar !== undefined && anchorPx !== undefined) {
      scrollBars = Math.max(0, anchorBar - anchorPx / next);
    }
    return setView(state, { pixelsPerBar: next, scrollBars });
  },
});

export const zoomBy = defineCommand({
  name: 'view.zoomBy',
  description: 'Multiply the arrangement zoom by a factor (wheel / pinch).',
  params: {
    factor: p.number({ min: 0.01, max: 100 }),
    anchorBar: p.optional(p.number()),
    anchorPx: p.optional(p.number()),
  },
  apply: (state, { factor, anchorBar, anchorPx }) =>
    setZoom.def.apply(state, {
      pixelsPerBar: clampZoom(state.view.pixelsPerBar * factor),
      ...(anchorBar !== undefined ? { anchorBar } : {}),
      ...(anchorPx !== undefined ? { anchorPx } : {}),
    }),
});

export const scrollTo = defineCommand({
  name: 'view.scrollTo',
  description: 'Scroll the arrangement so the given bar is at the left edge.',
  params: { bar: p.number({ min: 0 }) },
  transient: true,
  apply: (state, { bar }) => setView(state, { scrollBars: Math.max(0, bar) }),
});

export const scrollBy = defineCommand({
  name: 'view.scrollBy',
  description: 'Scroll the arrangement horizontally by a number of bars.',
  params: { bars: p.number() },
  transient: true,
  apply: (state, { bars }) =>
    setView(state, { scrollBars: Math.max(0, state.view.scrollBars + bars) }),
});

export const setAgentPanelOpen = defineCommand({
  name: 'view.setAgentPanelOpen',
  description: 'Show or collapse the agent panel.',
  params: { open: p.boolean() },
  apply: (state, { open }) => setView(state, { agentPanelOpen: open }),
});

export const setBrowserTab = defineCommand({
  name: 'view.setBrowserTab',
  description: 'Switch the browser sidebar tab.',
  params: { tab: p.enum(['instruments', 'loops', 'plugins', 'files']) },
  apply: (state, { tab }) => setView(state, { browserTab: tab }),
});

export const setEditorMode = defineCommand({
  name: 'view.setEditorMode',
  description: 'Switch the bottom editor between piano roll, score and step.',
  params: { mode: p.enum(['pianoRoll', 'score', 'step']) },
  apply: (state, { mode }) => setView(state, { editorMode: mode }),
});

export const setArrangeTool = defineCommand({
  name: 'view.setArrangeTool',
  description: 'Pick the arrangement tool.',
  params: { tool: p.enum(['pointer', 'pencil', 'scissors', 'grid']) },
  apply: (state, { tool }) => setView(state, { arrangeTool: tool }),
});
