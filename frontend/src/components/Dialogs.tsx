import { ShortcutList } from "./palette/Shortcuts";
import {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStore, useSession } from "../state/session";
import { native, type Params } from "../state/native";
import { PluginPanel } from "./PluginPanel";
import { AutomationPanel } from "./AutomationPanel";
import {
  exportProblem,
  exportSummary,
  type AudioExportReport,
} from "../state/export";
import { AgentSettings } from "./agent/AgentSettings";
import { ThemePicker } from "./settings/ThemePicker";

export function Modal({
  title,
  onClose,
  children,
  blocking = false,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  blocking?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const element = ref.current;
    if (blocking) element?.showModal();
    else element?.show();
    return () => element?.close();
  }, [blocking]);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  return (
    <dialog
      ref={ref}
      className="native-dialog"
      aria-label={title}
      style={{ translate: `${offset.x}px ${offset.y}px` }}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape" && !blocking) {
          onClose();
        }
      }}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <header
        onPointerDown={(e) => {
          if ((e.target as HTMLElement).closest("button")) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          drag.current = {
            x: e.clientX,
            y: e.clientY,
            left: offset.x,
            top: offset.y,
          };
        }}
        onPointerMove={(e) => {
          if (!drag.current || !e.currentTarget.hasPointerCapture(e.pointerId))
            return;
          const scale =
            Number(document.getElementById("root")?.style.zoom) || 1;
          setOffset({
            x: drag.current.left + (e.clientX - drag.current.x) / scale,
            y: drag.current.top + (e.clientY - drag.current.y) / scale,
          });
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        onPointerCancel={() => {
          drag.current = null;
        }}
      >
        <strong>{title}</strong>
        <button aria-label={`Close ${title}`} onClick={onClose}>
          ×
        </button>
      </header>
      <div className="dialog-content">{children}</div>
    </dialog>
  );
}
export function Dialogs() {
  const store = useStore();
  const ui = useSyncExternalStore(store.subscribeMeta, store.getUi);
  const name = useSession((s) => s.name);
  const close = (panel: string) =>
    store.fire("ui.showPanel", { panel, visible: false });
  return (
    <>
      {ui.settings && <Settings onClose={() => close("settings")} />}
      {ui.export && <Export onClose={() => close("export")} />}
      {ui.automation && <AutomationPanel onClose={() => close("automation")} />}
      {ui.recovery && <Recovery onClose={() => close("recovery")} />}
      {ui.help && (
        <Modal title="Working in Ondera" onClose={() => close("help")}>
          <p>
            Press ⌘/Ctrl P for the command palette: every menu command, by name.
            X swaps the region editor for the mixer.
          </p>
          <ShortcutList />
          <p>Ondera {store.version}</p>
        </Modal>
      )}
      {ui.pluginWindows?.map((id) => {
        for (const [trackId, strip] of Object.entries(
          store.document?.strips ?? {},
        )) {
          const slot = strip.inserts.findIndex((i) => i.id === id);
          if (
            slot >= 0 ||
            strip.synth?.id === id ||
            id === `${trackId}/synth/${strip.instrument}`
          )
            return (
              <PluginPanel
                key={id}
                id={id}
                trackId={trackId}
                slot={slot >= 0 ? slot : undefined}
                onClose={() => store.fire("web.closePlugin", { id })}
              />
            );
        }
        return null;
      })}
      {ui.prompt && (
        <Modal
          blocking
          title={`Save changes to ${name}?`}
          onClose={() => store.fire("web.confirm", { choice: "cancel" })}
        >
          <p>
            Your session has unsaved changes, or confirmation before quitting is
            enabled.
          </p>
          <footer>
            <button
              onClick={() => store.fire("web.confirm", { choice: "cancel" })}
            >
              Cancel
            </button>
            <button
              onClick={() => store.fire("web.confirm", { choice: "discard" })}
            >
              Don't save
            </button>
            <button
              className="primary"
              onClick={() => store.fire("web.confirm", { choice: "save" })}
            >
              Save
            </button>
          </footer>
        </Modal>
      )}
      {ui.error && (
        <Modal blocking title="Ondera" onClose={() => store.dismissError()}>
          <p role="alert">{ui.error}</p>
          <footer>
            <button onClick={() => store.dismissError()}>OK</button>
          </footer>
        </Modal>
      )}
    </>
  );
}
const titles: Record<string, string> = {
  general: "General",
  audio: "Audio & MIDI",
  interface: "Interface",
  agent: "Agent",
  plugins: "Plugins",
  control: "Control",
  updates: "Updates",
  about: "About",
};
const label = (key: string) =>
  key
    .replace(/([A-Z])/g, " $1")
    .replace(/^./, (s) => s.toUpperCase())
    .replace(/Api/g, "API")
    .replace(/Midi/g, "MIDI");
function Settings({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const [settings, setSettings] = useState<Record<string, Params>>({});
  const ui = useSyncExternalStore(store.subscribeMeta, store.getUi);
  const section = ui.settingsSection || "general";
  const [agentDirty, setAgentDirty] = useState(false);
  const [confirmClose, setConfirmClose] = useState(false);
  useEffect(() => {
    if (!agentDirty) setConfirmClose(false);
  }, [agentDirty]);
  const [devices, setDevices] = useState<{
    outputs: string[];
    inputs: string[];
    midiInputs: string[];
  }>({ outputs: [], inputs: [], midiInputs: [] });
  const refresh = () =>
    native<Record<string, Params>>("settings.get").then((value) => {
      setSettings(value);
    });
  useEffect(() => {
    void refresh().catch(store.reportError);
    void native<typeof devices>("audio.devices")
      .then(setDevices)
      .catch(store.reportError);
  }, []);
  const save = async (path: string, value: unknown) => {
    try {
      await store.run("settings.set", { path, value });
      await refresh();
    } catch {}
  };
  const closeSettingsAndChat = () => {
    onClose();
    store.fire("ui.showPanel", { panel: "agent", visible: true });
  };
  return (
    <Modal
      title="Settings"
      onClose={() => {
        if (agentDirty) setConfirmClose(true);
        else onClose();
      }}
    >
      {confirmClose && (
        <div className="settings-close-confirm" role="alert">
          <p>Discard unsaved agent connection changes?</p>
          <button onClick={() => setConfirmClose(false)}>Keep editing</button>
          <button onClick={onClose}>Discard changes</button>
        </div>
      )}
      <div className="settings-layout">
        <nav>
          {Object.entries(titles).map(([key, title]) => (
            <button
              key={key}
              className={section === key ? "selected" : ""}
              onClick={() =>
                store.fire("ui.showPanel", { panel: "settings", section: key })
              }
            >
              {title}
            </button>
          ))}
        </nav>
        <div className="settings-fields">
          {section === "about" && (
            <>
              <h2>Ondera {store.version}</h2>
              <p>Digital audio workstation for macOS, Linux and Windows.</p>
              <button onClick={() => store.fire("web.sdk")}>
                Native plugin SDK…
              </button>
            </>
          )}
          {settings.agent && (section === "agent" || agentDirty) && (
            <div hidden={section !== "agent"}>
              <AgentSettings
                key={String(settings.agent.provider)}
                settings={settings.agent}
                refresh={refresh}
                onStartChat={closeSettingsAndChat}
                onDirtyChange={setAgentDirty}
              />
            </div>
          )}
          {section !== "agent" &&
            Object.entries(
              settings[section === "updates" ? "general" : section] ?? {},
            )
              .filter(([key]) => {
                // The theme picker edits the mode together with the theme.
                if (["lastSession", "recentSessions", "mode"].includes(key))
                  return false;
                const update = [
                  "checkUpdatesOnStart",
                  "installUpdatesAutomatically",
                ].includes(key);
                return section === "updates" ? update : !update;
              })
              .map(([key, value]) => {
                const path = `${section === "updates" ? "general" : section}.${key}`;
                if (key === "appearance")
                  return (
                    <ThemePicker
                      key={path}
                      appearance={String(value ?? "")}
                      mode={String(settings.interface?.mode ?? "")}
                      onAppearance={(id) => void save(path, id)}
                      onMode={(mode) => void save("interface.mode", mode)}
                    />
                  );
                const options =
                  key === "provider"
                    ? ["codex", "claude", "anthropic", "openai", "compatible"]
                    : key === "outputDevice"
                      ? devices.outputs
                      : key === "inputDevice"
                        ? devices.inputs
                        : key === "midiInput"
                          ? devices.midiInputs
                          : null;
                if (value && typeof value === "object" && !Array.isArray(value))
                  return (
                    <fieldset key={key}>
                      <legend>{label(key)}</legend>
                      {Object.entries(value).map(([child, v]) => (
                        <label key={child}>
                          <span>{label(child)}</span>
                          <input
                            type="checkbox"
                            checked={Boolean(v)}
                            onChange={(e) =>
                              void save(`${path}.${child}`, e.target.checked)
                            }
                          />
                        </label>
                      ))}
                    </fieldset>
                  );
                return (
                  <label key={path}>
                    <span>{label(key)}</span>
                    {options ? (
                      <select
                        value={String(value ?? "")}
                        onChange={(e) =>
                          void save(path, e.target.value || null)
                        }
                      >
                        <option value="">System default</option>
                        {options.map((v) => (
                          <option key={v} value={v}>
                            {v}
                          </option>
                        ))}
                      </select>
                    ) : typeof value === "boolean" ? (
                      <input
                        type="checkbox"
                        checked={value}
                        onChange={(e) => void save(path, e.target.checked)}
                      />
                    ) : (
                      <input
                        key={`${path}:${JSON.stringify(value)}`}
                        type={
                          key.toLowerCase().includes("apikey")
                            ? "password"
                            : typeof value === "number"
                              ? "number"
                              : "text"
                        }
                        defaultValue={
                          Array.isArray(value)
                            ? value.join("; ")
                            : String(value ?? "")
                        }
                        onBlur={(e) => {
                          const next = Array.isArray(value)
                            ? e.target.value
                                .split(";")
                                .map((s) => s.trim())
                                .filter(Boolean)
                            : typeof value === "number"
                              ? Number(e.target.value)
                              : e.target.value;
                          if (JSON.stringify(next) !== JSON.stringify(value))
                            void save(path, next);
                        }}
                      />
                    )}
                  </label>
                );
              })}
          {section === "plugins" && (
            <button
              onClick={() => {
                void store
                  .run("plugin.scan")
                  .then(() => store.refreshPlugins())
                  .catch(store.reportError);
              }}
            >
              Scan plugins
            </button>
          )}
          {section === "updates" && (
            <div className="update-controls">
              <button onClick={() => store.fire("app.checkUpdates")}>
                Check for updates
              </button>
              {store.ui.update?.available && (
                <button onClick={() => store.fire("app.installUpdate")}>
                  Install {store.ui.update.available}
                </button>
              )}
              {store.ui.update?.installed && (
                <button
                  onClick={() => store.fire("web.file", { action: "relaunch" })}
                >
                  Relaunch
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
function Export({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const session = useSession((s) => s);
  const [rate, setRate] = useState(48000);
  const [format, setFormat] = useState("pcm24");
  const [tail, setTail] = useState(3);
  const [dither, setDither] = useState(true);
  const [range, setRange] = useState(false);
  const [start, setStart] = useState(1);
  const [end, setEnd] = useState(
    Math.max(1, ...session.clips.map((c) => c.startBar + c.lengthBars)) + 1,
  );
  const [includeEffects, setIncludeEffects] = useState(true);
  const [includeMaster, setIncludeMaster] = useState(false);
  const [stems, setStems] = useState(false);
  const [selected, setSelected] = useState(session.tracks.map((t) => t.id));
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState("");
  const liveSelected = selected.filter((id) =>
    session.tracks.some((track) => track.id === id),
  );
  const problem = exportProblem({
    range,
    start,
    end,
    tail,
    stems,
    trackCount: liveSelected.length,
  });
  const exporting = useRef(false);
  const run = async () => {
    if (exporting.current || problem) return;
    exporting.current = true;
    setReport("");
    setBusy(true);
    try {
      const path = await invoke<string | null>("daw_pick", {
        kind: stems ? "folder" : "wav",
        name: `${session.name}.wav`,
      });
      if (!path) return;
      const params: Params = {
        sampleRate: rate,
        format,
        tailSeconds: tail,
        dither: format !== "float32" && dither,
        ...(range ? { startBar: start - 1, endBar: end - 1 } : {}),
      };
      if (stems) {
        params.directory = `${path}/${session.name}-stems-${Date.now()}`;
        params.trackIds = liveSelected;
        params.includeEffects = includeEffects;
        params.includeMaster = includeMaster;
      } else params.path = path;
      const result = await store.run<AudioExportReport>(
        stems ? "session.exportStems" : "session.exportAudio",
        params,
      );
      setReport(exportSummary(result));
    } catch (error) {
      store.reportError(error);
    } finally {
      exporting.current = false;
      setBusy(false);
    }
  };
  return (
    <Modal
      title="Export audio"
      onClose={() => {
        if (!busy) onClose();
      }}
    >
      <fieldset className="settings-fields export-fields" disabled={busy}>
        <label>
          Sample rate
          <select
            value={rate}
            onChange={(e) => setRate(Number(e.target.value))}
          >
            {[44100, 48000, 96000].map((n) => (
              <option key={n} value={n}>
                {n / 1000} kHz
              </option>
            ))}
          </select>
        </label>
        <label>
          Format
          <select value={format} onChange={(e) => setFormat(e.target.value)}>
            <option value="pcm16">16-bit PCM</option>
            <option value="pcm24">24-bit PCM</option>
            <option value="float32">32-bit float</option>
          </select>
        </label>
        <label>
          Dither (PCM only)
          <input
            type="checkbox"
            disabled={format === "float32"}
            checked={format !== "float32" && dither}
            onChange={(e) => setDither(e.target.checked)}
          />
        </label>
        <label>
          Release tail (seconds)
          <input
            type="number"
            min={0}
            max={120}
            value={tail}
            onChange={(e) => setTail(Number(e.target.value))}
          />
        </label>
        <label>
          Bar range
          <input
            type="checkbox"
            checked={range}
            onChange={(e) => setRange(e.target.checked)}
          />
        </label>
        {range && (
          <>
            <label>
              Start bar
              <input
                type="number"
                min={1}
                value={start}
                onChange={(e) => setStart(Number(e.target.value))}
              />
            </label>
            <label>
              End bar (excluded)
              <input
                type="number"
                min={start + 0.01}
                value={end}
                onChange={(e) => setEnd(Number(e.target.value))}
              />
            </label>
          </>
        )}
        <label>
          Export track stems
          <input
            type="checkbox"
            checked={stems}
            onChange={(e) => setStems(e.target.checked)}
          />
        </label>
        {stems && (
          <>
            <label>
              Include track effects
              <input
                type="checkbox"
                checked={includeEffects}
                onChange={(e) => setIncludeEffects(e.target.checked)}
              />
            </label>
            <label>
              Include master processing
              <input
                type="checkbox"
                checked={includeMaster}
                onChange={(e) => setIncludeMaster(e.target.checked)}
              />
            </label>
          </>
        )}
        {stems &&
          session.tracks.map((t) => (
            <label key={t.id}>
              {t.name}
              <input
                type="checkbox"
                checked={selected.includes(t.id)}
                onChange={(e) =>
                  setSelected((ids) =>
                    e.target.checked
                      ? [...ids, t.id]
                      : ids.filter((id) => id !== t.id),
                  )
                }
              />
            </label>
          ))}
      </fieldset>
      {problem && <p role="status">{problem}</p>}
      <footer>
        <button disabled={busy} onClick={onClose}>
          Close
        </button>
        <button disabled={busy || Boolean(problem)} onClick={() => void run()}>
          {busy ? "Exporting…" : "Export…"}
        </button>
      </footer>
      {report && (
        <pre role="status" className="export-report">
          {report}
        </pre>
      )}
    </Modal>
  );
}
function Recovery({ onClose }: { onClose: () => void }) {
  const store = useStore();
  const [items, setItems] = useState<{ path: string; title: string }[]>([]);
  useEffect(() => {
    void native<{ snapshots: { path: string; title: string }[] }>(
      "session.snapshots",
    )
      .then((r) => setItems(r.snapshots))
      .catch(store.reportError);
  }, []);
  return (
    <Modal title="Recover session" onClose={onClose}>
      {items.length ? (
        items.map((item) => (
          <button
            className="recovery-item"
            key={item.path}
            onClick={() =>
              store.fire("session.restoreSnapshot", { path: item.path })
            }
          >
            {item.title}
            <small>{item.path}</small>
          </button>
        ))
      ) : (
        <p>No recovery snapshots.</p>
      )}
      <p>{store.ui.recoveryStatus}</p>
    </Modal>
  );
}
