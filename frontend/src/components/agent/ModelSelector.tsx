import { ProviderLogo } from "./ProviderLogo";
import { useRef, useState } from "react";
import { useStore } from "../../state/session";
import { isProvider, providers, type Provider } from "./connection";
import { effortName, useModels } from "./models";
import styles from "./AgentPanel.module.css";

export function ModelSelector({
  provider: currentProvider,
  model: currentModel,
  effort: currentEffort = "",
  disabled,
}: {
  provider: string;
  model: string;
  effort?: string;
  disabled: boolean;
}) {
  const store = useStore();
  const details = useRef<HTMLDetailsElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState<Provider>(
    isProvider(currentProvider) ? currentProvider : "codex",
  );
  const [model, setModel] = useState(currentModel);
  const [effort, setEffort] = useState(currentEffort);
  const [saving, setSaving] = useState(false);
  const locked = useRef(false);
  const [error, setError] = useState("");
  const catalog = useModels();
  const selected = catalog.groups
    .find((g) => g.provider === provider)
    ?.models.find((m) => m.id === model);
  const selectedKey = `${provider}:${model}`;
  const label =
    catalog.groups
      .find((g) => g.provider === currentProvider)
      ?.models.find((m) => m.id === currentModel)?.name ||
    currentModel ||
    "Service default";
  return (
    <details
      className={styles.modelPicker}
      ref={details}
      onKeyDown={(event) => {
        if (event.key === "Escape" && details.current?.open) {
          event.stopPropagation();
          details.current.open = false;
          details.current.querySelector("summary")?.focus();
        }
      }}
      onToggle={() => {
        const expanded = Boolean(details.current?.open);
        setOpen(expanded);
        if (expanded) {
          setProvider(isProvider(currentProvider) ? currentProvider : "codex");
          setModel(currentModel);
          setEffort(currentEffort);
          setError("");
          setQuery("");
          void catalog.refresh();
        }
      }}
    >
      <summary aria-label="Choose agent model">
        <ProviderLogo provider={currentProvider} model={currentModel} />
        <span className={styles.modelLabel}>{label}</span>{" "}
        <span>{effortName(currentEffort) || "Auto"}⌄</span>
      </summary>
      {open && (
        <form
          className={styles.modelMenu}
          onSubmit={async (e) => {
            e.preventDefault();
            if (disabled || locked.current) return;
            locked.current = true;
            setSaving(true);
            setError("");
            try {
              await store.request("agent.configure", {
                provider,
                model: model.trim(),
                reasoningEffort: effort,
              });
              if (details.current) details.current.open = false;
            } catch (reason) {
              setError(String(reason));
            } finally {
              locked.current = false;
              setSaving(false);
            }
          }}
        >
          <fieldset disabled={disabled || saving}>
            <legend>Your connected models</legend>
            <label>
              Find a model
              <input
                type="search"
                name="model-search"
                autoComplete="off"
                spellCheck={false}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search your models…"
              />
            </label>
            <div
              className={styles.modelList}
              role="radiogroup"
              aria-label="Agent model"
              aria-busy={catalog.loading}
            >
              {!selected && !query && (
                <label className={styles.modelOption}>
                  <input
                    type="radio"
                    name="agent-model"
                    checked
                    readOnly
                    value={selectedKey}
                  />
                  <ProviderLogo provider={provider} model={model} />
                  <span>
                    {model || "Service default"}
                    <small>Current selection</small>
                  </span>
                </label>
              )}
              {catalog.groups
                .filter((g) => g.models.length)
                .map((g) => {
                  const matches = g.models.filter((m) =>
                    `${m.name} ${m.id} ${providers[g.provider].name} ${g.label}`
                      .toLowerCase()
                      .includes(query.toLowerCase()),
                  );
                  if (!matches.length) return null;
                  return (
                    <section key={g.provider}>
                      <h3>
                        <ProviderLogo provider={g.provider} />
                        {providers[g.provider].name}
                        <small>{g.label}</small>
                      </h3>
                      {matches.map((m) => (
                        <label
                          key={m.id}
                          className={styles.modelOption}
                          data-selected={
                            provider === g.provider && model === m.id
                          }
                        >
                          <input
                            type="radio"
                            name="agent-model"
                            value={`${g.provider}:${m.id}`}
                            checked={provider === g.provider && model === m.id}
                            onChange={() => {
                              setProvider(g.provider);
                              setModel(m.id);
                              setEffort("");
                            }}
                          />
                          <ProviderLogo provider={g.provider} model={m.id} />
                          <span>
                            {m.name}
                            {m.name !== m.id && <small>{m.id}</small>}
                          </span>
                        </label>
                      ))}
                    </section>
                  );
                })}
              {query &&
                !catalog.groups.some((g) =>
                  g.models.some((m) =>
                    `${m.name} ${m.id} ${providers[g.provider].name} ${g.label}`
                      .toLowerCase()
                      .includes(query.toLowerCase()),
                  ),
                ) && <p>No matching models.</p>}
            </div>
            <label>
              Reasoning
              <select
                aria-label="Reasoning effort"
                value={effort}
                onChange={(e) => setEffort(e.target.value)}
              >
                <option value="">Provider default</option>
                {(selected?.efforts ?? []).map((level) => (
                  <option key={level} value={level}>
                    {effortName(level)}
                  </option>
                ))}
                {effort && !selected?.efforts.includes(effort) && (
                  <option value={effort}>{effortName(effort)} · saved</option>
                )}
              </select>
            </label>
            {selected && !selected.efforts.length && (
              <p>
                This provider does not advertise adjustable thinking modes for
                this model.
              </p>
            )}
            <p role="status">
              {catalog.loading
                ? "Fetching models from your connections…"
                : "Models reported by your connected accounts and APIs."}
            </p>
            {catalog.groups
              .filter((g) => g.error)
              .map((g) => (
                <p key={g.provider}>
                  {providers[g.provider].name}: {g.error}
                </p>
              ))}
            {(catalog.error || error) && (
              <p role="alert">{error || catalog.error}</p>
            )}
            <button
              type="button"
              className="m-button"
              disabled={catalog.loading}
              onClick={() => void catalog.refresh()}
            >
              Refresh models
            </button>{" "}
            <button
              type="submit"
              disabled={catalog.loading}
              className="m-button aero-primary"
            >
              {saving ? "Saving…" : "Use this model"}
            </button>
            <p>
              <button
                type="button"
                className="m-button"
                onClick={() =>
                  store.fire("ui.showPanel", {
                    panel: "settings",
                    section: "agent",
                  })
                }
              >
                Manage connections
              </button>
            </p>
          </fieldset>
        </form>
      )}
    </details>
  );
}
