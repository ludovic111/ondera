import { useEffect } from "react";
import { useStore } from "./session";
import { actions, runAction, type ActionId } from "./actions";
import { matchesShortcut, type Shortcut } from "./shortcuts";
import {
  bounceSession,
  importAudioFiles,
  newSession,
  openSession,
  saveSession,
} from "./document";
import type { SessionStore } from "@ondera/core";

/** File operations are not commands (they talk to the host), so they get their own table. */
const FILE_SHORTCUTS: {
  shortcut: Shortcut;
  run: (store: SessionStore) => void;
}[] = [
  { shortcut: { key: "n", meta: true }, run: (store) => newSession(store) },
  {
    shortcut: { key: "o", meta: true },
    run: (store) => void openSession(store),
  },
  {
    shortcut: { key: "s", meta: true },
    run: (store) => void saveSession(store),
  },
  {
    shortcut: { key: "s", meta: true, shift: true },
    run: (store) => void saveSession(store, true),
  },
  {
    shortcut: { key: "i", meta: true },
    run: (store) => void importAudioFiles(store),
  },
  {
    shortcut: { key: "b", meta: true },
    run: (store) => void bounceSession(store),
  },
];

const ORDERED = Object.values(actions).filter((a) => a.shortcut !== undefined);

function isTextTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return (
    !!el &&
    (el.tagName === "INPUT" ||
      el.tagName === "TEXTAREA" ||
      el.tagName === "SELECT" ||
      el.isContentEditable)
  );
}

/**
 * Global shortcuts. One table (actions.ts) feeds both this handler and the
 * menus, so what a menu shows is what the keyboard does.
 */
export function useKeyboardShortcuts(): void {
  const store = useStore();
  useEffect(() => {
    let octave = 0;
    const held = new Map<string, number>();
    const offsets: Record<string, number> = {
      a: 0,
      w: 1,
      s: 2,
      e: 3,
      d: 4,
      f: 5,
      t: 6,
      g: 7,
      y: 8,
      h: 9,
      u: 10,
      j: 11,
      k: 12,
      o: 13,
      l: 14,
      p: 15,
      ";": 16,
    };
    const release = () => {
      held.clear();
      store.fire("note.releaseAll");
      store.fire("web.gesture", { active: false });
    };
    const up = (e: KeyboardEvent) => {
      const pitch = held.get(e.code);
      if (pitch !== undefined) {
        held.delete(e.code);
        store.fire("web.liveNote", { pitch, on: false });
      }
    };
    const begin = () => store.fire("web.gesture", { active: true });
    const end = () => store.fire("web.gesture", { active: false });
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      if (store.ui.prompt || store.ui.error) return;
      const target = e.target instanceof HTMLElement ? e.target : null;
      if (target?.closest("dialog")) return;
      if (e.repeat && e.code === "Space") return;
      if (isTextTarget(e.target)) return;
      if ((e.key === "Enter" || e.key === " ") && target?.closest("button"))
        return;
      if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey) {
        if (e.key === ",") {
          e.preventDefault();
          store.fire("ui.showPanel", { panel: "settings", visible: true });
          return;
        }
        if (e.key.toLowerCase() === "k") {
          e.preventDefault();
          release();
          store.fire("ui.musicalTyping", { enabled: !store.ui.musicalTyping });
          return;
        }
      }
      if (store.ui.musicalTyping && !e.metaKey && !e.ctrlKey && !e.altKey) {
        const key = e.key.toLowerCase();
        if (key === "z" || key === "x") {
          e.preventDefault();
          if (!e.repeat)
            octave = Math.max(-3, Math.min(3, octave + (key === "z" ? -1 : 1)));
          return;
        }
        if (key in offsets) {
          e.preventDefault();
          if (!e.repeat && !held.has(e.code)) {
            const pitch = 60 + octave * 12 + offsets[key];
            held.set(e.code, pitch);
            store.fire("web.liveNote", { pitch, on: true });
          }
          return;
        }
      }
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
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", release);
    const focus = (event: FocusEvent) => {
      if (
        isTextTarget(event.target) ||
        (event.target instanceof HTMLElement && event.target.closest("dialog"))
      )
        release();
    };
    window.addEventListener("focusin", focus);
    window.addEventListener("pointerdown", begin, true);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", release);
      window.removeEventListener("focusin", focus);
      window.removeEventListener("pointerdown", begin, true);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      release();
    };
  }, [store]);
}
