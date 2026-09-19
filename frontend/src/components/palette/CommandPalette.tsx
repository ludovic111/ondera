import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "../../state/session";
import type { SessionStore } from "@ondera/core";
import { actions } from "../../state/actions";
import {
  MENU_TITLES,
  actionItem,
  buildMenu,
  type MenuEntry,
  type MenuItem,
} from "../../state/menus";
import styles from "./CommandPalette.module.css";

interface Entry extends MenuItem {
  group: string;
}

/** Score a query against a label: prefix beats word start beats substring beats scattered letters. */
export function matchScore(label: string, query: string): number {
  const l = label.toLowerCase();
  const q = query.trim().toLowerCase();
  if (!q) return 1;
  if (l.startsWith(q)) return 4;
  if (l.includes(` ${q}`)) return 3;
  if (l.includes(q)) return 2;
  let i = 0;
  for (const ch of l) if (ch === q[i]) i++;
  return i === q.length ? 1 : 0;
}

/** Menu rows first, then any action the menus do not list (transport, tools, editor modes). */
export function paletteEntries(store: SessionStore): Entry[] {
  const seen = new Set<string>();
  const out: Entry[] = [];
  const add = (item: MenuEntry, group: string) => {
    if (item.separator || !item.onSelect || seen.has(item.label)) return;
    seen.add(item.label);
    out.push({ ...item, group });
  };
  for (const group of MENU_TITLES)
    for (const item of buildMenu(group, store)) add(item, group);
  const listed = new Set(out.map((e) => e.shortcut).filter(Boolean));
  for (const def of Object.values(actions)) {
    const item = actionItem(store, def.id);
    if (!item.separator && item.shortcut && listed.has(item.shortcut)) continue;
    add(item, "Action");
  }
  return out;
}

/**
 * Every menu command in one searchable list. It is built from the menus, so
 * what it runs, enables and ticks is exactly what the menu bar does.
 */
export function CommandPalette({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const list = useRef<HTMLUListElement>(null);
  const entries = useMemo(() => paletteEntries(store), [store]);
  const results = useMemo(
    () =>
      entries
        .map((entry) => ({
          entry,
          score: matchScore(`${entry.label} ${entry.group}`, query),
        }))
        .filter((r) => r.score > 0)
        .sort(
          (a, b) =>
            b.score - a.score ||
            Number(a.entry.disabled ?? false) -
              Number(b.entry.disabled ?? false),
        )
        .map((r) => r.entry),
    [entries, query],
  );
  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    list.current?.children[active]?.scrollIntoView?.({ block: "nearest" });
  }, [active]);
  const run = (entry: Entry | undefined) => {
    if (!entry || entry.disabled) return;
    onClose();
    entry.onSelect?.();
  };
  return (
    <div className={styles.scrim} onPointerDown={onClose}>
      <div
        className={styles.palette}
        role="dialog"
        aria-label="Command palette"
        onPointerDown={(e) => e.stopPropagation()}
      >
        <input
          autoFocus
          className={styles.input}
          placeholder="Type a command…"
          aria-label="Search commands"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Escape") onClose();
            else if (e.key === "Enter") run(results[active]);
            else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const step = e.key === "ArrowDown" ? 1 : -1;
              setActive(
                (i) =>
                  (i + step + results.length) % Math.max(1, results.length),
              );
            }
          }}
        />
        <ul ref={list} className={styles.list} role="listbox">
          {results.map((entry, i) => (
            <li
              key={entry.label}
              role="option"
              aria-selected={i === active}
              aria-disabled={entry.disabled}
              className={styles.row}
              onPointerMove={() => setActive(i)}
              onClick={() => run(entry)}
            >
              <span className={styles.check}>{entry.checked ? "✓" : ""}</span>
              <span className={styles.label}>{entry.label}</span>
              <span className={styles.group}>{entry.group}</span>
              <kbd className={styles.key}>{entry.shortcut ?? ""}</kbd>
            </li>
          ))}
          {results.length === 0 && (
            <li className={styles.none}>No command matches “{query}”.</li>
          )}
        </ul>
      </div>
    </div>
  );
}
