import { useEffect, useState } from "react";
import { native } from "../../state/native";
import { useStore } from "../../state/session";
import styles from "./ExternalAgents.module.css";

interface Client {
  id: string;
  name: string;
  how: string;
  text: string;
  file: string | null;
  link: boolean;
}
interface McpInfo {
  bridgeEnabled: boolean;
  clients: Client[];
}

/**
 * Connect an agent that lives outside ryolune (Claude Code, Codex, Cursor, VS Code, Claude
 * Desktop, Gemini CLI…) to this window over MCP: one ready configuration per client, copied
 * or installed from a link. The configurations come from `agent.mcp`.
 */
export function ExternalAgents() {
  const store = useStore();
  const [info, setInfo] = useState<McpInfo | null>(null);
  const [chosen, setChosen] = useState("claude-code");
  const [copied, setCopied] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    native<McpInfo>("agent.mcp")
      .then((value) => {
        if (live && Array.isArray(value?.clients)) setInfo(value);
      })
      .catch((reason) => live && setError(String(reason)));
    return () => {
      live = false;
    };
  }, []);
  const client =
    info?.clients.find((c) => c.id === chosen) ?? info?.clients[0] ?? null;
  const copy = async (text: string, id: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(id);
      window.setTimeout(() => setCopied(""), 1600);
    } catch {
      setError("Copy did not work here: select the text and copy it.");
    }
  };
  return (
    <section className={styles.external} aria-label="Use another agent">
      <h2>Use another agent</h2>
      <p>
        Claude Code, Codex, Cursor and any app that speaks MCP can work on this
        song from outside. They get the same commands as the built-in agent, and
        every change they make lands in Undo.
      </p>
      {info && !info.bridgeEnabled && (
        <div className={styles.warning} role="status">
          The local connection is off, so outside agents cannot reach this
          window.
          <button
            type="button"
            onClick={() =>
              store.fire("ui.showPanel", {
                panel: "settings",
                section: "control",
              })
            }
          >
            Turn it on
          </button>
        </div>
      )}
      {error && <p role="alert">{error}</p>}
      {info && client && (
        <>
          <div
            className={styles.clients}
            role="tablist"
            aria-label="Agent to connect"
          >
            {info.clients.map((c) => (
              <button
                key={c.id}
                type="button"
                role="tab"
                aria-selected={c.id === client.id}
                className={c.id === client.id ? styles.on : ""}
                onClick={() => setChosen(c.id)}
              >
                {c.name}
              </button>
            ))}
          </div>
          <div className={styles.recipe} role="tabpanel">
            <p>{client.how}</p>
            {client.file && (
              <p className={styles.file}>
                <span>File</span> <code>{client.file}</code>
              </p>
            )}
            <pre className={styles.code}>{client.text}</pre>
            <div className={styles.actions}>
              {client.link && (
                <button
                  type="button"
                  className="primary"
                  onClick={() =>
                    void store
                      .request("agent.openClient", { client: client.id })
                      .catch((reason) => setError(String(reason)))
                  }
                >
                  Add to {client.name.replace(/ \(.*\)$/, "")}
                </button>
              )}
              <button
                type="button"
                onClick={() => void copy(client.text, client.id)}
              >
                {copied === client.id ? "Copied" : "Copy"}
              </button>
            </div>
            <p className={styles.note}>
              Keep ryolune open while the agent works: it edits this window.
            </p>
          </div>
        </>
      )}
    </section>
  );
}
