import type { Session } from '../model/types';
import type { ParamSchema, ParamsOf } from './schema';

/** A command instance: what clients (GUI, CLI, MCP) send to the store. */
export interface Command<P = Record<string, unknown>> {
  name: string;
  params: P;
}

export interface CommandDef<S extends ParamSchema = ParamSchema> {
  name: string;
  description: string;
  params: S;
  /**
   * Transient commands are high-frequency state updates (engine ticks,
   * scroll) that are not worth recording in history and are not undoable.
   */
  transient: boolean;
  apply: (state: Session, params: ParamsOf<S>) => Session;
}

/** A callable creator that also carries its definition. */
export interface CommandCreator<S extends ParamSchema> {
  (params: ParamsOf<S>): Command<ParamsOf<S>>;
  def: CommandDef<S>;
}

export function defineCommand<const S extends ParamSchema>(spec: {
  name: string;
  description: string;
  params: S;
  transient?: boolean;
  apply: (state: Session, params: ParamsOf<S>) => Session;
}): CommandCreator<S> {
  const def: CommandDef<S> = {
    name: spec.name,
    description: spec.description,
    params: spec.params,
    transient: spec.transient ?? false,
    apply: spec.apply,
  };
  const creator = ((params: ParamsOf<S>) => ({ name: spec.name, params })) as CommandCreator<S>;
  creator.def = def;
  return creator;
}
