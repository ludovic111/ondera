import { useEffect, useState } from "react";
import { Modal } from "./Dialogs";
import { useStore } from "../state/session";
import { native } from "../state/native";
interface Parameter {
  id: number;
  name: string;
  min: number;
  max: number;
  value: number;
  default: number;
  unit: string;
  steps: number;
  labels: string[];
  logarithmic: boolean;
}
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
  const refresh = async () => {
    try {
      const result = await native<{
        pluginId: string;
        parameters: Parameter[];
      }>("strip.parameters", target);
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
  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  }, [id]);
  const change = (p: Parameter, value: number) => {
    setParameters((list) =>
      list.map((item) => (item.id === p.id ? { ...item, value } : item)),
    );
    store.fire("strip.setParameter", { ...target, parameterId: p.id, value });
  };
  return (
    <Modal
      title={pluginId.replace(/^stock:/, "") || "Plugin parameters"}
      onClose={onClose}
    >
      <div className="plugin-toolbar">
        <button
          onClick={() =>
            store.fire("ui.openPluginWindow", { ...target, native: true })
          }
        >
          Open plugin window
        </button>
        <select
          aria-label="Preset"
          defaultValue=""
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
      </div>
      {error && <p role="status">{error}</p>}
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
            <input
              aria-label={`${p.name} value`}
              type="number"
              min={p.min}
              max={p.max}
              step="any"
              value={Number(p.value.toFixed(4))}
              onChange={(e) => change(p, Number(e.target.value))}
            />
            <small>{p.unit}</small>
            <button
              title={`Automate ${p.name}`}
              onClick={() => {
                void store
                  .run("automation.create", {
                    target: "pluginParameter",
                    ...target,
                    parameterId: p.id,
                  })
                  .then(() =>
                    store.fire("ui.showPanel", { panel: "automation" }),
                  )
                  .catch(store.reportError);
              }}
            >
              A
            </button>
          </label>
        ))}
      </div>
    </Modal>
  );
}
