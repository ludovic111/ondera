/** A keyboard shortcut. `key` matches KeyboardEvent.key case-insensitively, except 'Space'. */
export interface Shortcut {
  key: string;
  meta?: boolean;
  shift?: boolean;
  alt?: boolean;
}

const KEY_GLYPH: Record<string, string> = {
  Space: "Space",
  Enter: "↩",
  Backspace: "⌫",
  Delete: "⌦",
  ArrowLeft: "←",
  ArrowRight: "→",
  ArrowUp: "↑",
  ArrowDown: "↓",
  Escape: "Esc",
  Home: "↖",
};

export function formatShortcut(s: Shortcut): string {
  const mods = `${s.alt ? "⌥" : ""}${s.shift ? "⇧" : ""}${s.meta ? "⌘" : ""}`;
  const key =
    KEY_GLYPH[s.key] ?? (s.key.length === 1 ? s.key.toUpperCase() : s.key);
  return mods + key;
}

export function matchesShortcut(e: KeyboardEvent, s: Shortcut): boolean {
  const meta = e.metaKey || e.ctrlKey;
  if (meta !== Boolean(s.meta)) return false;
  if (e.shiftKey !== Boolean(s.shift)) return false;
  if (e.altKey !== Boolean(s.alt)) return false;
  if (s.key === "Space")
    return e.code === "Space" || e.key === " " || e.key === "Spacebar";
  if (s.key === "Enter") return isEnterKey(e);
  if (e.key.toLowerCase() === s.key.toLowerCase()) return true;
  // With Option held, macOS reports the composed character (⌥A is "å"): fall back to the
  // key's position for letters and digits.
  return (
    Boolean(s.alt) &&
    /^[a-z0-9]$/i.test(s.key) &&
    e.code === (/\d/.test(s.key) ? "Digit" : "Key") + s.key.toUpperCase()
  );
}

/** Enter as reported by different hosts and keypads. */
export function isEnterKey(e: { key: string; code?: string }): boolean {
  return (
    e.key === "Enter" ||
    e.key === "Return" ||
    e.code === "Enter" ||
    e.code === "NumpadEnter"
  );
}
