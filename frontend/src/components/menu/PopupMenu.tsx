import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent,
} from "react";
import { createPortal } from "react-dom";
import type { MenuEntry } from "../../state/menus";
import { size } from "../../theme/tokens";
import styles from "./PopupMenu.module.css";

export interface PopupMenuProps {
  items: MenuEntry[];
  /** Viewport position of the menu's top-left corner. */
  x: number;
  y: number;
  onClose: () => void;
  /**
   * The control that opened the menu. Pressing it again closes the menu, and the click that
   * follows is swallowed so it does not open the menu straight back up.
   */
  anchor?: Element | null;
}

/**
 * A raised graphite card with one row per item. Used for title-bar menus,
 * context menus and value pickers. Closes on outside click, Escape, blur or
 * after a selection.
 */
export function PopupMenu({ items, x, y, onClose, anchor }: PopupMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(
    items.findIndex((i) => !i.separator && !i.disabled),
  );
  useEffect(() => {
    ref.current?.focus();
  }, []);
  const [pos, setPos] = useState({ x, y });

  // Keep the menu inside the window.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const nx = Math.max(
      size.menuPad,
      Math.min(x, window.innerWidth - r.width - size.menuPad),
    );
    const ny = Math.max(
      size.menuPad,
      Math.min(y, window.innerHeight - r.height - size.menuPad),
    );
    setPos({ x: nx, y: ny });
  }, [x, y, items]);

  useEffect(() => {
    const onDown = (e: PointerEvent) => {
      if (!ref.current || ref.current.contains(e.target as Node)) return;
      if (anchor?.contains(e.target as Node)) swallowNextClick(anchor);
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose, anchor]);

  const stop = (e: MouseEvent) => e.stopPropagation();

  return createPortal(
    <div
      ref={ref}
      className={styles.menu}
      data-motion="pop"
      role="menu"
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          e.stopPropagation();
          const available = items
            .map((entry, i) => (!entry.separator && !entry.disabled ? i : -1))
            .filter((i) => i >= 0);
          const index = available.indexOf(active);
          const next =
            available[
              (index + (e.key === "ArrowDown" ? 1 : -1) + available.length) %
                available.length
            ];
          if (next !== undefined) {
            setActive(next);
            ref.current?.children[next]?.scrollIntoView({ block: "nearest" });
          }
        } else if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          const item = items[active];
          if (item && !item.separator && !item.disabled) {
            item.onSelect?.();
            onClose();
          }
        }
      }}
      style={{ left: pos.x, top: pos.y }}
      onContextMenu={(e) => e.preventDefault()}
      onClick={stop}
    >
      {items.map((it, i) =>
        it.separator ? (
          <div key={i} className={styles.separator} />
        ) : (
          <div
            key={i}
            role="menuitem"
            aria-disabled={it.disabled}
            onMouseEnter={() => setActive(i)}
            className={`${styles.item} ${active === i ? styles.active : ""} ${it.disabled ? styles.disabled : ""}`}
            onClick={() => {
              if (it.disabled) return;
              it.onSelect?.();
              onClose();
            }}
          >
            <span className={styles.check}>{it.checked ? "✓" : ""}</span>
            <span className={styles.label}>{it.label}</span>
            {it.shortcut && (
              <span className={styles.shortcut}>{it.shortcut}</span>
            )}
          </div>
        ),
      )}
    </div>,
    document.body,
  );
}

/** Position + items for a menu a component is currently showing. */
export interface MenuState {
  x: number;
  y: number;
  items: MenuEntry[];
  anchor?: Element | null;
}

/** Anchor a menu to the bottom-left of an element. */
export function anchorBelow(el: HTMLElement): {
  x: number;
  y: number;
  anchor: HTMLElement;
} {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.bottom + 2, anchor: el };
}

/** Drop the click that completes a press on `anchor`, if it comes soon. */
function swallowNextClick(anchor: Element) {
  const swallow = (e: Event) => {
    window.removeEventListener("click", swallow, true);
    if (anchor.contains(e.target as Node)) {
      e.stopPropagation();
      e.preventDefault();
    }
  };
  window.addEventListener("click", swallow, true);
  window.setTimeout(
    () => window.removeEventListener("click", swallow, true),
    1000,
  );
}
