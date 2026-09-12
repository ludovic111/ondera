import type { CommandCreator, CommandDef } from './commands/define';
import type { ParamSchema } from './commands/schema';
import * as transport from './commands/transport';
import * as track from './commands/track';
import * as clip from './commands/clip';
import * as view from './commands/view';
import * as agent from './commands/agent';

/**
 * Every command the product exposes, grouped by family. This object is the
 * public API: the GUI, the CLI and the MCP server all go through it.
 */
export const commands = { transport, track, clip, view, agent } as const;

type AnyCreator = CommandCreator<ParamSchema>;

function collect(): Map<string, CommandDef> {
  const map = new Map<string, CommandDef>();
  for (const family of Object.values(commands)) {
    for (const creator of Object.values(family)) {
      if (typeof creator !== 'function' || !('def' in creator)) continue;
      const def = (creator as AnyCreator).def;
      if (map.has(def.name)) throw new Error(`duplicate command name "${def.name}"`);
      map.set(def.name, def as CommandDef);
    }
  }
  return map;
}

/** Name → definition. Introspectable: iterate this to build a CLI or MCP tool list. */
export const registry: ReadonlyMap<string, CommandDef> = collect();

export function getCommandDef(name: string): CommandDef {
  const def = registry.get(name);
  if (!def) throw new Error(`unknown command "${name}"`);
  return def;
}

/** Plain-data description of the registry, suitable for printing or serialising. */
export function describeCommands(): { name: string; description: string; params: ParamSchema }[] {
  return [...registry.values()].map(({ name, description, params }) => ({ name, description, params }));
}
