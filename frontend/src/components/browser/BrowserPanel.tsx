import { useMemo, useState, type MouseEvent } from "react";
import {
  commands,
  secondsToBars,
  type BrowserItem,
  type BrowserTab,
} from "@ondera/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { importAudioFiles } from "../../state/document";
import { playheadBar } from "../../state/actions";
import { SegmentedControl } from "../primitives/SegmentedControl";
import { Button } from "../primitives/Button";
import { CapsLabel } from "../primitives/CapsLabel";
import {
  ChevronRightIcon,
  PlaySmallIcon,
  SearchIcon,
  StarIcon,
} from "../primitives/Icons";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import type { MenuEntry } from "../../state/menus";
import styles from "./BrowserPanel.module.css";

const TABS: { id: BrowserTab; label: string }[] = [
  { id: "instruments", label: "Instr" },
  { id: "loops", label: "Loops" },
  { id: "plugins", label: "Plugins" },
  { id: "files", label: "Files" },
];

const PLACEHOLDER: Record<BrowserTab, string> = {
  instruments: "Search instruments",
  loops: "Search loops",
  plugins: "Search plugins",
  files: "Search files",
};

const HINT: Record<BrowserTab, string> = {
  instruments: "Double-click: load on the selected MIDI track, or add one",
  loops: "Double-click: add a MIDI loop at the playhead",
  plugins: "Double-click: insert on the selected track",
  files:
    "Double-click: place at the playhead · drop files on the window to import",
};

/** Folders the user has closed, remembered between launches. */
const CLOSED_KEY = "ondera.browser.closedFolders";
function readClosed(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(CLOSED_KEY) ?? "[]"));
  } catch {
    return new Set();
  }
}

export function BrowserPanel() {
  const store = useStore();
  const dispatch = useDispatch();
  const tab = useSession((s) => s.view.browserTab);
  const groups = useSession((s) => s.browser[s.view.browserTab]);
  const selection = useSession((s) => s.view.browserSelection);
  const [query, setQuery] = useState("");
  const [closed, setClosed] = useState(readClosed);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const pluginTab = tab === "plugins" || tab === "instruments";
  // Only the folder the person just opened animates its rows in; the list as a whole, a
  // search, or a rescan must not ripple.
  const [revealed, setRevealed] = useState<string | null>(null);
  const toggleFolder = (key: string) =>
    setClosed((prior) => {
      const next = new Set(prior);
      if (next.delete(key)) setRevealed(key);
      else {
        next.add(key);
        setRevealed(null);
      }
      try {
        localStorage.setItem(CLOSED_KEY, JSON.stringify([...next]));
      } catch {
        // Private mode: the folders simply reopen next launch.
      }
      return next;
    });
  const folderNames = useMemo(
    () => [
      ...new Set(
        [
          ...store.getState().browser.instruments,
          ...store.getState().browser.plugins,
        ]
          .filter((g) => g.kind === "folder")
          .map((g) => g.name),
      ),
    ],
    [groups],
  );
  const setFavorite = (item: BrowserItem, favorite: boolean) =>
    void store
      .run("plugin.setFavorite", { pluginId: item.id, favorite })
      .then(() => store.refreshPlugins())
      .catch(store.reportError);
  const setFolder = (item: BrowserItem, folder?: string) =>
    void store
      .run("plugin.setFolder", {
        pluginId: item.id,
        ...(folder ? { folder } : {}),
      })
      .then(() => store.refreshPlugins())
      .catch(store.reportError);
  const openItemMenu = (e: MouseEvent, item: BrowserItem) => {
    if (!pluginTab || !item.id) return;
    e.preventDefault();
    const items: MenuEntry[] = [
      {
        label: tab === "instruments" ? "Load on track" : "Insert on track",
        onSelect: () => void activate(item),
      },
      { separator: true },
      {
        label: item.favorite ? "Remove from Favourites" : "Add to Favourites",
        onSelect: () => setFavorite(item, !item.favorite),
      },
      { separator: true },
      ...folderNames
        .filter((name) => name !== item.folder)
        .map((name) => ({
          label: `Move to ${name}`,
          onSelect: () => setFolder(item, name),
        })),
      {
        label: "Move to new folder…",
        onSelect: () => {
          const name = window.prompt("Folder name")?.trim();
          if (name) setFolder(item, name);
        },
      },
      { label: "Return to automatic folder", onSelect: () => setFolder(item) },
    ];
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const q = query.trim().toLowerCase();
  const visible = groups
    .map((g) => ({
      ...g,
      items: q
        ? g.items.filter(
            (it) =>
              it.name.toLowerCase().includes(q) ||
              it.meta.toLowerCase().includes(q) ||
              (it.folder ?? "").toLowerCase().includes(q),
          )
        : g.items,
    }))
    .filter((g) => g.items.length > 0);

  const activate = async (item: BrowserItem) => {
    const selected = store
      .getState()
      .tracks.find((t) => t.id === store.getState().view.selectedTrackId);
    if (tab === "loops") {
      store.fire("clip.addLoop", { name: item.name });
      return;
    }
    if (tab === "files") {
      const s = store.getState();
      const source = item.id ? s.sources[item.id] : undefined;
      if (!source) return;
      try {
        const track =
          selected?.kind === "audio"
            ? selected
            : await store.run<{ id: string }>("track.add", {
                kind: "audio",
                name: source.name,
              });
        const { numerator, denominator } = s.transport.timeSignature;
        const barSeconds =
          (60 / s.transport.tempo) * numerator * (4 / denominator);
        await store.run("clip.create", {
          trackId: track.id,
          sourceId: source.id,
          name: source.name,
          startBar: playheadBar(s),
          lengthBars: Math.max(0.25, source.durationSeconds / barSeconds),
        });
        store.receive(await store.run("web.document"));
      } catch (error) {
        store.reportError(error);
      }
      return;
    }
    const plugin = store.plugins.find((p) => p.id === item.id);
    if (!plugin) return;
    try {
      if (tab === "instruments") {
        const track =
          selected?.kind === "midi"
            ? selected
            : await store.run<{ id: string }>("track.add", {
                kind: "midi",
                name: item.name,
              });
        await store.run("strip.setPlugin", {
          trackId: track.id,
          pluginId: plugin.id,
        });
      } else if (store.getState().view.selectedTrackId) {
        const trackId = store.getState().view.selectedTrackId!;
        const inserts = store.getState().strips[trackId]?.inserts ?? [];
        const slot = inserts.findIndex((i) => i.state === "empty");
        if (slot < 0) {
          store.reportError(
            "All eight inserts are occupied. Remove an effect first.",
          );
          return;
        }
        await store.run("strip.setPlugin", {
          trackId,
          slot,
          pluginId: plugin.id,
        });
      }
    } catch (error) {
      store.reportError(error);
    }
  };

  const preview = () => {
    const s = store.getState();
    const track = s.tracks.find(
      (t) => t.id === s.view.selectedTrackId && t.kind === "midi",
    );
    if (track)
      store.fire("note.preview", {
        trackId: track.id,
        pitch: 60,
        velocity: 100,
      });
    else store.reportError("Select an instrument track to preview");
  };

  return (
    <div className={styles.panel}>
      <div className={styles.tabs} data-surface="browser-header">
        <SegmentedControl
          items={TABS}
          value={tab}
          stretch
          onChange={(id) => {
            dispatch(commands.view.setBrowserTab({ tab: id }));
            setQuery("");
          }}
        />
      </div>
      <div className={`${styles.search} m-groove-alt`}>
        <SearchIcon />
        <input
          className={styles.searchInput}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={PLACEHOLDER[tab]}
          spellCheck={false}
        />
      </div>
      <div className={styles.list} title={HINT[tab]}>
        {visible.length === 0 && <div className={styles.empty}>No matches</div>}
        {visible.map((g) => {
          const key = `${tab}/${g.name}`;
          const open = q !== "" || !closed.has(key);
          return (
            <div key={g.name} className={styles.group}>
              {pluginTab ? (
                <button
                  className={styles.folder}
                  aria-expanded={open}
                  onClick={() => toggleFolder(key)}
                >
                  <span
                    data-motion="turn"
                    className={`${styles.chevron} ${open ? styles.chevronOpen : ""}`}
                  >
                    <ChevronRightIcon />
                  </span>
                  {g.color && (
                    <span
                      className={styles.folderTab}
                      style={{ background: g.color }}
                    />
                  )}
                  <span className={styles.folderName}>{g.name}</span>
                  <span className={styles.count}>{g.items.length}</span>
                </button>
              ) : (
                <CapsLabel style={{ padding: "8px 8px 4px" }}>
                  {g.name}
                </CapsLabel>
              )}
              {open && (
                <div
                  className={styles.rows}
                  data-motion={revealed === key ? "reveal" : undefined}
                >
                  {g.items.map((it) => (
                    <div
                      key={it.id ?? it.name}
                      className={`${styles.item} ${pluginTab ? styles.filed : ""} ${(it.id ?? it.name) === selection ? styles.highlighted : ""}`}
                      onClick={() =>
                        dispatch(
                          commands.view.setBrowserSelection({
                            name: it.id ?? it.name,
                          }),
                        )
                      }
                      onDoubleClick={() => void activate(it)}
                      onContextMenu={(e) => openItemMenu(e, it)}
                    >
                      <span
                        className={`${styles.dot} m-swatch`}
                        style={{
                          background: it.color ?? "var(--color-neutral-dot)",
                        }}
                      />
                      <span className={styles.itemName}>{it.name}</span>
                      <span className={styles.meta}>{it.meta}</span>
                      {pluginTab && it.id && (
                        <button
                          className={`${styles.star} ${it.favorite ? styles.starred : ""}`}
                          title={
                            it.favorite
                              ? "Remove from Favourites"
                              : "Add to Favourites"
                          }
                          aria-pressed={it.favorite ?? false}
                          onClick={(e) => {
                            e.stopPropagation();
                            setFavorite(it, !it.favorite);
                          }}
                          onDoubleClick={(e) => e.stopPropagation()}
                        >
                          <StarIcon filled={it.favorite ?? false} />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
        {tab === "files" ? (
          <button
            className="m-button"
            onClick={() =>
              void importAudioFiles(store).catch(store.reportError)
            }
          >
            Import audio…
          </button>
        ) : (
          (tab === "plugins" || tab === "instruments") && (
            <button
              className="m-button"
              onClick={() =>
                void store
                  .run("plugin.scan")
                  .then(() => store.refreshPlugins())
                  .catch(store.reportError)
              }
            >
              Scan plugins
            </button>
          )
        )}
      </div>
      {menu && <PopupMenu {...menu} onClose={() => setMenu(null)} />}
      <div className={styles.footer}>
        <Button
          size="icon"
          className={styles.preview}
          onClick={preview}
          title="Audition the selected track's instrument"
        >
          <PlaySmallIcon />
        </Button>
        <span>Preview</span>
        <span className={styles.previewName}>
          {store.plugins.find((p) => p.id === selection)?.name ??
            selection ??
            "—"}
        </span>
      </div>
    </div>
  );
}

export { secondsToBars };
