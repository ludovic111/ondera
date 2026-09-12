import { useEffect } from 'react';
import { useStore } from './session';
import { actions, runAction, type ActionId } from './actions';
import { matchesShortcut, type Shortcut } from './shortcuts';
import { bounceSession, importAudioFiles, newSession, openSession, saveSession } from './document';
import type { SessionStore } from '@ondera/core';

/** File operations are not commands (they talk to the host), so they get their own table. */
const FILE_SHORTCUTS: { shortcut: Shortcut; run: (store: SessionStore) => void }[] = [
  { shortcut: { key: 'n', meta: true }, run: (store) => newSession(store) },
  { shortcut: { key: 'o', meta: true }, run: (store) => void openSession(store) },
  { shortcut: { key: 's', meta: true }, run: (store) => void saveSession(store) },
  { shortcut: { key: 's', meta: true, shift: true }, run: (store) => void saveSession(store, true) },
  { shortcut: { key: 'i', meta: true }, run: (store) => void importAudioFiles(store) },
  { shortcut: { key: 'b', meta: true }, run: (store) => void bounceSession(store) },
];

const ORDERED = Object.values(actions).filter((a) => a.shortcut !== undefined);

function isTextTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
}

/**
 * Global shortcuts. One table (actions.ts) feeds both this handler and the
 * menus, so what a menu shows is what the keyboard does.
 */
export function useKeyboardShortcuts(): void {
  const store = useStore();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat && e.code === 'Space') return;
      if (isTextTarget(e.target)) return;
      for (const f of FILE_SHORTCUTS) {
        if (!matchesShortcut(e, f.shortcut)) continue;
        e.preventDefault();
        f.run(store);
        return;
      }
      for (const def of ORDERED) {
        if (!def.shortcut || !matchesShortcut(e, def.shortcut)) continue;
        e.preventDefault();
        runAction(store, def.id as ActionId);
        return;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [store]);
}
