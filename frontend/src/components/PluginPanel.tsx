import { useEffect, useRef, useState } from "react";
import { Modal } from "./Dialogs";
import { useStore } from "../state/session";
import { native } from "../state/native";
import { PluginFace, type Parameter } from "./plugin/PluginFace";
import { familyVar } from "../theme/families";

export function PluginPanel({
  id,
  trackId,
  slot,
  onClose,
}: {
  id: string;
  trackId: string;
  slot?: number;
  onClose: () => void;
}) {
  const store = useStore();
  const [parameters, setParameters] = useState<Parameter[]>([]);
  const [pluginId, setPluginId] = useState("");
  const [presets, setPresets] = useState<{ name: string; factory: boolean }[]>(
    [],
  );
  const [presetName, setPresetName] = useState("");
  const [error, setError] = useState("");
  const target = { trackId, ...(slot === undefined ? {} : { slot }) };
  // The refresh below is set up once per plugin: read where the plugin sits now, not where
  // it sat when the window opened (Move up/down, or a slot removed above it).
  const where = useRef(target);
  where.current = target;
  const refresh = async () => {
    try {
      const result = await native<{
        pluginId: string;
        parameters: Parameter[];
      }>("strip.parameters", where.current);
      setPluginId(result.pluginId);
      setParameters(result.parameters);
      const list = await native<{ presets: typeof presets }>("preset.list", {
        pluginId: result.pluginId,
      });
      setPresets(list.presets);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };
  // Parameter values are document state: re-read them when the document moves
  // (automation write, undo, an agent edit) rather than on a timer.
  useEffect(() => {
    void refresh();
    let seen = store.document;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = store.subscribe(() => {
      if (store.document === seen) return;
      seen = store.document;
      clearTimeout(timer);
      timer = setTimeout(() => void refresh(), 150);
    });
    return () => {
      clearTimeout(timer);
      unsubscribe();
    };
  }, [id]);
  const change = (p: Parameter, value: number) => {
    setParameters((list) =>
      list.map((item) => (item.id === p.id ? { ...item, value } : item)),
    );
    store.fire("strip.setParameter", { ...target, parameterId: p.id, value });
  };
  const automate = (p: Parameter) => {
    void store
      .run("automation.create", {
        target: "pluginParameter",
        ...target,
        parameterId: p.id,
      })
      .then(() => store.fire("ui.showPanel", { panel: "automation" }))
      .catch(store.reportError);
  };
  const stock = pluginId.startsWith("stock:");
  const name = pluginId.replace(/^stock:/, "");
  const toolbar = (
    <>
      {!stock && (
        <button
          onClick={() =>
            store.fire("ui.openPluginWindow", { ...target, native: true })
          }
        >
          Open plugin window
        </button>
      )}
      <select
        aria-label="Preset"
        value=""
        onChange={(e) => {
          void store
            .run("preset.load", { ...target, name: e.target.value })
            .then(refresh)
            .catch(store.reportError);
        }}
      >
        <option value="" disabled>
          Preset…
        </option>
        {presets.map((p) => (
          <option key={p.name}>{p.name}</option>
        ))}
      </select>
      <input
        aria-label="Preset name"
        placeholder="Preset name"
        value={presetName}
        onChange={(e) => setPresetName(e.target.value)}
      />
      <button
        disabled={!presetName.trim()}
        onClick={() => {
          void store
            .run("preset.save", { ...target, name: presetName })
            .then(refresh)
            .catch(store.reportError);
        }}
      >
        Save preset
      </button>
    </>
  );
  // The engine files every plugin in a sound folder; the panel wears that family's colour.
  const folder = store.plugins.find((p) => p.id === pluginId)?.folder;
  return (
    <Modal title={name || "Plugin parameters"} onClose={onClose}>
      {error && <p role="status">{error}</p>}
      {stock ? (
        <PluginFace
          name={name}
          category={folder ?? "Plugin"}
          tint={folder ? familyVar(folder) : undefined}
          parameters={parameters}
          onChange={change}
          onAutomate={automate}
          toolbar={toolbar}
        />
      ) : (
        <>
          <div className="plugin-toolbar">{toolbar}</div>
          <div className="parameter-list">
            {parameters.map((p) => (
              <label key={p.id}>
                <span>{p.name}</span>
                <input
                  aria-label={p.name}
                  type="range"
                  min={p.min}
                  max={p.max}
                  step={
                    p.steps > 1
                      ? (p.max - p.min) / (p.steps - 1)
                      : (p.max - p.min) / 1000
                  }
                  value={p.value}
                  onChange={(e) => change(p, Number(e.target.value))}
                  onDoubleClick={() => change(p, p.default)}
                />
                <ValueField parameter={p} onCommit={(v) => change(p, v)} />
                <small>{p.unit}</small>
                <button
                  title={`Automate ${p.name}`}
                  onClick={() => automate(p)}
                >
                  A
                </button>
              </label>
            ))}
          </div>
        </>
      )}
    </Modal>
  );
}

/**
 * A parameter typed as a number. It commits on Enter or when it loses focus, clamped to the
 * parameter's range: committing each keystroke sent 0 for an emptied field and out-of-range
 * values halfway through typing ("5" on the way to "50").
 */
export function ValueField({
  parameter: p,
  onCommit,
}: {
  parameter: Parameter;
  onCommit: (value: number) => void;
}) {
  const shown = String(Number(p.value.toFixed(4)));
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text === null) return;
    setText(null);
    const value = Number(text);
    if (text.trim() === "" || !Number.isFinite(value)) return;
    const clamped = Math.min(p.max, Math.max(p.min, value));
    if (clamped !== p.value) onCommit(clamped);
  };
  return (
    <input
      aria-label={`${p.name} value`}
      type="number"
      min={p.min}
      max={p.max}
      step="any"
      value={text ?? shown}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape") setText(null);
      }}
    />
  );
}
