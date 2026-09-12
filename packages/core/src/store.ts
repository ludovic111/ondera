import type { Command } from './commands/define';
import { validateParams } from './commands/schema';
import { getCommandDef } from './registry';
import type { Session } from './model/types';

export type Listener = (state: Session, command: Command) => void;

export interface HistoryEntry {
  command: Command;
  /** Monotonic sequence number. */
  seq: number;
}

/**
 * The single place session state changes. Clients call dispatch(); nothing
 * else writes to state. Framework-agnostic: React binds with
 * useSyncExternalStore, a CLI would call dispatch() directly.
 */
export class SessionStore {
  private state: Session;
  private listeners = new Set<Listener>();
  private history: HistoryEntry[] = [];
  private seq = 0;

  constructor(initial: Session) {
    this.state = initial;
  }

  getState = (): Session => this.state;

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /**
   * Validate, apply, record, notify. Throws on unknown command or bad params;
   * state is untouched when it throws.
   */
  dispatch = (command: Command<any>): Session => {
    const def = getCommandDef(command.name);
    validateParams(command.name, def.params, command.params);
    const next = def.apply(this.state, command.params as never);
    if (next === this.state) return next;
    this.state = next;
    if (!def.transient) this.history.push({ command, seq: ++this.seq });
    for (const l of this.listeners) l(next, command);
    return next;
  };

  getHistory(): readonly HistoryEntry[] {
    return this.history;
  }
}
