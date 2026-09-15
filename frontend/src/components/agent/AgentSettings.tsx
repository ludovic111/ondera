import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStore } from "../../state/session";
import type { Params } from "../../state/native";
import {
  canChat,
  isProvider,
  providers,
  useAgentConnection,
} from "./connection";
import { effortName, useModels } from "./models";
import styles from "./AgentSettings.module.css";

const permissionLabels: Record<string, [string, string]> = {
  fileOperations: [
    "Save, import and export files",
    "Let the agent work with files when you ask.",
  ],
  transport: [
    "Play and record",
    "Let the agent control playback and recording.",
  ],
  replaceSession: [
    "Replace the current project",
    "Allow opening another project or starting a new one.",
  ],
  settings: [
    "Change settings",
    "Allow changes to preferences and audio devices.",
  ],
  appControl: ["Quit and update Ondera", "Allow application control."],
};

export function AgentSettings({
  settings,
  refresh,
  onStartChat,
  onDirtyChange,
}: {
  settings: Params;
  refresh: () => Promise<void>;
  onStartChat: () => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const store = useStore();
  const catalog = useModels();
  useEffect(() => {
    void catalog.refresh();
  }, [catalog.refresh]);
  const agent = useSyncExternalStore(store.subscribeMeta, store.getAgent);
  const provider = isProvider(settings.provider) ? settings.provider : "codex";
  const service = providers[provider];
  const account = provider === "codex" || provider === "claude";
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const {
    connection,
    checking,
    error: checkError,
    check,
  } = useAgentConnection(provider);
  const [effort, setEffort] = useState(String(settings.reasoningEffort ?? ""));
  const [model, setModel] = useState(String(settings.model ?? ""));
  const [url, setUrl] = useState(String(settings.compatibleBaseUrl ?? ""));
  const [instructions, setInstructions] = useState(
    String(settings.instructions ?? ""),
  );
  const [maxTokens, setMaxTokens] = useState(
    Number(settings.maxOutputTokens ?? 4096),
  );
  const [maxRounds, setMaxRounds] = useState(
    Number(settings.maxToolRounds ?? 48),
  );
  const [key, setKey] = useState("");
  const [executable, setExecutable] = useState(
    String(settings[`${provider}Executable`] ?? ""),
  );
  const keyPath =
    provider === "anthropic"
      ? "anthropicApiKey"
      : provider === "openai"
        ? "openaiApiKey"
        : "compatibleApiKey";
  const hasKey = Boolean(settings[keyPath]);
  const unsaved =
    effort !== String(settings.reasoningEffort ?? "") ||
    instructions !== String(settings.instructions ?? "") ||
    maxTokens !== Number(settings.maxOutputTokens ?? 4096) ||
    maxRounds !== Number(settings.maxToolRounds ?? 48) ||
    model !== String(settings.model ?? "") ||
    key !== "" ||
    (provider === "compatible" &&
      url !== String(settings.compatibleBaseUrl ?? "")) ||
    (account && executable !== String(settings[`${provider}Executable`] ?? ""));
  useEffect(() => {
    onDirtyChange?.(unsaved);
  }, [unsaved, onDirtyChange]);
  const save = async (entries: [string, unknown][]) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      for (const [path, value] of entries)
        await store.run("settings.set", { path: `agent.${path}`, value });
      await refresh();
      setKey("");
      setModel(model.trim());
      setUrl(url.trim());
      setExecutable(executable.trim());
      setNotice("Connection settings saved.");
      await check();
    } catch (reason) {
      setError(String(reason));
    } finally {
      locked.current = false;
      setBusy(false);
    }
  };
  const signIn = async () => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    setNotice(
      "Finish signing in in your browser, then return here. This can take a few minutes.",
    );
    try {
      await invoke<string>("daw_signin", { provider, status: false });
      setNotice("Sign-in finished. Checking your account…");
      await check();
    } catch {
      setError(
        "Sign-in did not finish. Try again, or follow the installation help and check your connection once signed in.",
      );
    } finally {
      setNotice("");
      locked.current = false;
      setBusy(false);
    }
  };
  return (
    <section className={styles.setup} aria-label="Agent connection">
      <div>
        <h2>Make music with an agent</h2>
        <p>
          Choose a service, connect it, then describe what you want to hear.
        </p>
      </div>
      <label>
        <span>AI service</span>
        <select
          value={provider}
          disabled={busy || unsaved || agent.status.running}
          onChange={(event) => void save([["provider", event.target.value]])}
        >
          {Object.entries(providers).map(([id, item]) => (
            <option key={id} value={id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      <p>{service.description}</p>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save([
            ["model", model.trim()],
            ["reasoningEffort", effort],
            ["instructions", instructions],
            ["maxOutputTokens", maxTokens],
            ["maxToolRounds", maxRounds],
            ...(account
              ? [
                  [`${provider}Executable`, executable.trim()] as [
                    string,
                    unknown,
                  ],
                ]
              : []),
            ...(provider === "compatible"
              ? [["compatibleBaseUrl", url.trim()] as [string, unknown]]
              : []),
            ...(key.trim() ? [[keyPath, key.trim()] as [string, unknown]] : []),
          ]);
        }}
      >
        <fieldset disabled={busy || agent.status.running}>
          <legend>{account ? "Your account" : "Connection details"}</legend>
          {provider === "compatible" && (
            <>
              <label>
                <span>Server address</span>
                <input
                  type="url"
                  required
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="http://localhost:1234/v1"
                />
              </label>
              <p>
                Use the OpenAI-compatible address from your server, including
                /v1 when required.
              </p>
            </>
          )}
          {!account && (
            <>
              <label>
                <span>
                  API key{provider === "compatible" ? " (optional)" : ""}
                </span>
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={key}
                  onChange={(e) => setKey(e.target.value)}
                  placeholder={
                    hasKey
                      ? "Key saved · enter a replacement"
                      : "Paste your API key"
                  }
                />
              </label>
              {hasKey && (
                <button
                  type="button"
                  disabled={unsaved}
                  onClick={() => void save([[keyPath, ""]])}
                >
                  Remove saved key
                </button>
              )}
              {provider === "compatible" && (
                <label>
                  <span>Model name</span>
                  <input
                    required
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="Model loaded in your server"
                    spellCheck={false}
                  />
                </label>
              )}
            </>
          )}
          <label>
            <span>Reasoning effort</span>
            <select value={effort} onChange={(e) => setEffort(e.target.value)}>
              {[
                ...new Set([
                  "",
                  effort,
                  ...(catalog.groups
                    .find((g) => g.provider === provider)
                    ?.models.find((m) => m.id === model)?.efforts ?? []),
                ]),
              ].map((id) => (
                <option key={id} value={id}>
                  {id ? effortName(id) : "Provider default"}
                </option>
              ))}
            </select>
          </label>
          <details>
            <summary>Advanced connection settings</summary>
            <div className={styles.advanced}>
              {provider !== "compatible" && (
                <label>
                  <span>Model (optional)</span>
                  <input
                    value={model}
                    onChange={(e) => setModel(e.target.value)}
                    placeholder="Use the service default"
                    spellCheck={false}
                  />
                </label>
              )}
              {account && (
                <label>
                  <span>Companion executable (optional)</span>
                  <input
                    value={executable}
                    onChange={(e) => setExecutable(e.target.value)}
                    placeholder="Find automatically"
                    spellCheck={false}
                  />
                </label>
              )}
              <label>
                <span>Reply token limit</span>
                <input
                  type="number"
                  min={256}
                  max={128000}
                  required
                  value={maxTokens}
                  onChange={(e) => setMaxTokens(Number(e.target.value))}
                />
              </label>
              <label>
                <span>Tool round limit</span>
                <input
                  type="number"
                  min={1}
                  max={500}
                  required
                  value={maxRounds}
                  onChange={(e) => setMaxRounds(Number(e.target.value))}
                />
              </label>
              <label>
                <span>Standing instructions</span>
                <textarea
                  maxLength={20000}
                  value={instructions}
                  onChange={(e) => setInstructions(e.target.value)}
                  placeholder="Your preferred style, workflow or language…"
                />
              </label>
              <p>
                Leave optional fields empty to use automatic settings. Changing
                service resets the model to its default.
              </p>
            </div>
          </details>
          {(unsaved || !account) && (
            <button type="submit" className="primary" disabled={!unsaved}>
              {busy ? "Saving…" : "Save connection"}
            </button>
          )}
        </fieldset>
      </form>
      {unsaved && (
        <p role="status">
          Save your connection changes before switching service or connecting.
        </p>
      )}
      <div className={styles.connection}>
        <strong>
          {checking
            ? "Checking connection…"
            : connection?.state === "signedIn"
              ? "Account connected"
              : connection?.state === "configured"
                ? "Ready to try"
                : "Connect your agent"}
        </strong>
        <p role="status">
          {notice ||
            (checking
              ? "Checking this computer. No message is sent to an AI model."
              : connection?.message)}
        </p>
        {(error || checkError) && <p role="alert">{error || checkError}</p>}
        <div className={styles.actions}>
          {connection?.state === "bridgeDisabled" && (
            <button
              onClick={() =>
                store.fire("ui.showPanel", {
                  panel: "settings",
                  section: "control",
                })
              }
            >
              Open connection settings
            </button>
          )}
          {account &&
            connection &&
            ["signInRequired", "signedIn"].includes(connection.state) && (
              <button
                disabled={busy || checking || unsaved}
                onClick={() => void signIn()}
              >
                {connection.state === "signedIn"
                  ? "Sign in again"
                  : `Sign in with ${service.name}`}
              </button>
            )}
          <button
            disabled={busy || checking || unsaved}
            onClick={() => void check()}
          >
            Check connection
          </button>
          {canChat(connection) && (
            <button
              className="primary"
              disabled={busy || checking || unsaved}
              onClick={onStartChat}
            >
              Start chatting
            </button>
          )}
        </div>
        {service.help && (
          <button
            className={styles.help}
            onClick={() =>
              void invoke("daw_agent_help", { provider }).catch((reason) =>
                setError(String(reason)),
              )
            }
          >
            {account ? "Installation and sign-in help ↗" : "Get an API key ↗"}
          </button>
        )}
      </div>
      <p className={styles.privacy}>
        {service.destination} The agent can edit this project; edits appear in
        Changes and can be undone.
      </p>
      <details>
        <summary>What the agent can do</summary>
        <div className={styles.advanced}>
          {Object.entries(
            (settings.permissions ?? {}) as Record<string, boolean>,
          ).map(([name, value]) => {
            const copy = permissionLabels[name];
            if (!copy) return null;
            return (
              <label key={name}>
                <span>
                  {copy[0]}
                  <small>{copy[1]}</small>
                </span>
                <input
                  type="checkbox"
                  checked={value}
                  disabled={busy || unsaved || agent.status.running}
                  onChange={(e) =>
                    void save([[`permissions.${name}`, e.target.checked]])
                  }
                />
              </label>
            );
          })}
        </div>
      </details>
    </section>
  );
}
