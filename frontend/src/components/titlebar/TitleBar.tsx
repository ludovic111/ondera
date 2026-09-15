import { useState, useSyncExternalStore, type MouseEvent } from "react";
import { useSession, useStore } from "../../state/session";
import { buildMenu, MENU_TITLES, type MenuTitle } from "../../state/menus";
import { anchorBelow, PopupMenu, type MenuState } from "../menu/PopupMenu";
import styles from "./TitleBar.module.css";

/** Window chrome. Native traffic lights overlay the left inset on macOS. Menu labels open popup menus. */
export function TitleBar() {
  const store = useStore();
  const ui = useSyncExternalStore(store.subscribeMeta, store.getUi);
  const name = useSession((s) => s.name);
  const isMac = store.platform === "macos";
  const [menu, setMenu] = useState<(MenuState & { title: MenuTitle }) | null>(
    null,
  );

  const open = (title: MenuTitle, el: HTMLElement) =>
    setMenu({ title, items: buildMenu(title, store), ...anchorBelow(el) });
  const onClick = (title: MenuTitle) => (e: MouseEvent<HTMLSpanElement>) => {
    if (menu?.title === title) setMenu(null);
    else open(title, e.currentTarget);
  };
  // Sliding across titles while one menu is open switches menus, like a native menu bar.
  const onEnter = (title: MenuTitle) => (e: MouseEvent<HTMLSpanElement>) => {
    if (menu && menu.title !== title) open(title, e.currentTarget);
  };

  return (
    <div
      data-tauri-drag-region
      data-surface="titlebar"
      className={`${styles.bar} ${isMac ? styles.mac : ""}`}
    >
      <div className={styles.menu}>
        {MENU_TITLES.map((m) => (
          <span
            key={m}
            className={`${styles.menuTitle} ${menu?.title === m ? styles.menuOpen : ""}`}
            onClick={onClick(m)}
            onMouseEnter={onEnter(m)}
          >
            {m}
          </span>
        ))}
      </div>
      <div data-tauri-drag-region className={styles.title}>
        {name}
        {ui.dirty ? " *" : ""}
      </div>
      <span className={styles.status}>{ui.status}</span>
      {menu && (
        <PopupMenu
          items={menu.items}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
