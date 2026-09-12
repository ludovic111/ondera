import { defineCommand } from './define';

/**
 * Undo and redo are meta-commands: the store intercepts them and restores a
 * snapshot instead of calling apply(). They live in the registry so the CLI
 * and MCP server expose them like everything else.
 */
export const undo = defineCommand({
  name: 'history.undo',
  description: 'Undo the last recorded command.',
  params: {},
  transient: true,
  apply: (state) => state,
});

export const redo = defineCommand({
  name: 'history.redo',
  description: 'Redo the last undone command.',
  params: {},
  transient: true,
  apply: (state) => state,
});
