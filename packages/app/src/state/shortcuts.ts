/** A keyboard shortcut. `key` matches KeyboardEvent.key case-insensitively, except 'Space'. */
export interface Shortcut {
  key: string;
  meta?: boolean;
  shift?: boolean;
  alt?: boolean;
}

const KEY_GLYPH: Record<string, string> = {
  Space: 'Space',
  Enter: '↩',
  Backspace: '⌫',
  Delete: '⌦',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  Escape: 'Esc',
  Home: '↖',
};

export function formatShortcut(s: Shortcut): string {
  const mods = `${s.alt ? '⌥' : ''}${s.shift ? '⇧' : ''}${s.meta ? '⌘' : ''}`;
  const key = KEY_GLYPH[s.key] ?? (s.key.length === 1 ? s.key.toUpperCase() : s.key);
  return mods + key;
}

export function matchesShortcut(e: KeyboardEvent, s: Shortcut): boolean {
  const meta = e.metaKey || e.ctrlKey;
  if (meta !== Boolean(s.meta)) return false;
  if (e.shiftKey !== Boolean(s.shift)) return false;
  if (e.altKey !== Boolean(s.alt)) return false;
  if (s.key === 'Space') return e.code === 'Space' || e.key === ' ' || e.key === 'Spacebar';
  return e.key.toLowerCase() === s.key.toLowerCase();
}
