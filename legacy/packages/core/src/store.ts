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

/** Undo stack depth. Snapshots share structure, so this is cheap. */
const MAX_UNDO = 200;

/**
 * Fields that describe "now" rather than the document. Undo restores the
 * document but leaves these as they are, so undoing an edit while playing
 * does not jump the playhead or stop the transport.
 */
function carryLive(restored: Session, current: Session): Session {
  return {
    ...restored,
    transport: {
      ...restored.transport,
      playing: current.transport.playing,
      recording: current.transport.recording,
      positionBeats: current.transport.positionBeats,
    },
    view: { ...restored.view, scrollBars: current.view.scrollBars },
    agent: { ...restored.agent, draft: current.agent.draft },
    meters: current.meters,
  };
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
  private past: Session[] = [];
  private future: Session[] = [];

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
   * state is untouched when it throws. history.undo / history.redo are
   * intercepted and restore snapshots instead of calling apply().
   */
  dispatch = (command: Command<any>): Session => {
    const def = getCommandDef(command.name);
    validateParams(def.name, def.params, command.params);
    if (def.name === 'history.undo') return this.undo();
    if (def.name === 'history.redo') return this.redo();
    if (def.name === 'session.load') return this.load((command.params as { token: string }).token);
    const prev = this.state;
    const next = def.apply(prev, command.params);
    if (next === prev) return prev;
    if (!def.transient) {
      this.history.push({ command, seq: ++this.seq });
      this.past.push(prev);
      if (this.past.length > MAX_UNDO) this.past.shift();
      this.future = [];
    }
    this.state = next;
    for (const l of this.listeners) l(next, command);
    return next;
  };

  private staged = new Map<string, Session>();

  /**
   * Sessions cannot travel inside command params (params are scalars so the
   * registry stays serialisable), so the host stages the object and passes
   * the token through session.load.
   */
  stage = (session: Session): string => {
    const token = `staged-${++this.seq}`;
    this.staged.set(token, session);
    return token;
  };

  private load(token: string): Session {
    const session = this.staged.get(token);
    if (!session) throw new Error(`session.load: unknown token "${token}"`);
    this.staged.delete(token);
    this.past = [];
    this.future = [];
    this.state = session;
    for (const l of this.listeners) l(session, { name: 'session.load', params: { token } });
    return session;
  }

  canUndo = (): boolean => this.past.length > 0;
  canRedo = (): boolean => this.future.length > 0;

  undo = (): Session => {
    const snapshot = this.past.pop();
    if (!snapshot) return this.state;
    this.future.push(this.state);
    return this.restore(snapshot, { name: 'history.undo', params: {} });
  };

  redo = (): Session => {
    const snapshot = this.future.pop();
    if (!snapshot) return this.state;
    this.past.push(this.state);
    return this.restore(snapshot, { name: 'history.redo', params: {} });
  };

  private restore(snapshot: Session, command: Command): Session {
    const next = carryLive(snapshot, this.state);
    this.state = next;
    for (const l of this.listeners) l(next, command);
    return next;
  }

  getHistory(): readonly HistoryEntry[] {
    return this.history;
  }
}
