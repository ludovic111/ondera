import { useState } from "react";
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
import { PlaySmallIcon, SearchIcon } from "../primitives/Icons";
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

export function BrowserPanel() {
  const store = useStore();
  const dispatch = useDispatch();
  const tab = useSession((s) => s.view.browserTab);
  const groups = useSession((s) => s.browser[s.view.browserTab]);
  const selection = useSession((s) => s.view.browserSelection);
  const [query, setQuery] = useState("");

  const q = query.trim().toLowerCase();
  const visible = groups
    .map((g) => ({
      ...g,
      items: q
        ? g.items.filter(
            (it) =>
              it.name.toLowerCase().includes(q) ||
              it.meta.toLowerCase().includes(q),
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
        {visible.map((g) => (
          <div key={g.name}>
            <CapsLabel style={{ padding: "8px 8px 4px" }}>{g.name}</CapsLabel>
            {g.items.map((it) => (
              <div
                key={it.id ?? it.name}
                className={`${styles.item} ${(it.id ?? it.name) === selection ? styles.highlighted : ""}`}
                onClick={() =>
                  dispatch(
                    commands.view.setBrowserSelection({
                      name: it.id ?? it.name,
                    }),
                  )
                }
                onDoubleClick={() => void activate(it)}
              >
                <span
                  className={`${styles.dot} m-swatch`}
                  style={{ background: it.color ?? "var(--color-neutral-dot)" }}
                />
                {it.name}
                <span className={styles.meta}>{it.meta}</span>
              </div>
            ))}
          </div>
        ))}
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
