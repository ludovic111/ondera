import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useStore } from "../../state/session";
import { Button } from "../primitives/Button";
import styles from "./AgentPanel.module.css";
export function AgentPanel() {
  const store = useStore();
  const agent = useSyncExternalStore(store.subscribeMeta, store.getAgent);
  const [draft, setDraft] = useState("");
  const [tab, setTab] = useState<"conversation" | "changes">("conversation");
  const end = useRef<HTMLDivElement>(null);
  useEffect(
    () => end.current?.scrollIntoView({ block: "nearest" }),
    [agent.transcript],
  );
  const send = () => {
    if (!draft.trim() || agent.status.running) return;
    store.fire("agent.send", { prompt: draft });
    setDraft("");
  };
  return (
    <aside className={styles.panel} aria-label="Agent">
      <div className={styles.header}>
        <span className={`${styles.dot} m-led-accent`} />
        <strong className={styles.title}>Agent</strong>
        <span className={styles.status}>{agent.status.provider}</span>
        <Button
          size="icon"
          onClick={() => store.fire("ui.showPanel", { panel: "settings" })}
          title="Agent settings"
        >
          ⚙
        </Button>
        <Button
          size="icon"
          className={styles.close}
          onClick={() =>
            store.fire("ui.showPanel", { panel: "agent", visible: false })
          }
          title="Collapse agent"
        >
          ›
        </Button>
      </div>
      <div className="agent-tabs">
        <button
          className={tab === "conversation" ? "m-segment-selected" : ""}
          onClick={() => setTab("conversation")}
        >
          Conversation
        </button>
        <button
          className={tab === "changes" ? "m-segment-selected" : ""}
          onClick={() => setTab("changes")}
        >
          Changes · {agent.changes.length}
        </button>
      </div>
      <div className="agent-conversation">
        {tab === "conversation"
          ? agent.transcript.entries.map((entry, i) => (
              <article className="agent-message" key={i}>
                <span className="caps">
                  {entry.role === "user"
                    ? "You"
                    : entry.role === "assistant"
                      ? "Agent"
                      : (entry.tool?.name ?? entry.role)}
                </span>
                <div>{entry.text}</div>
                {entry.tool && (
                  <details>
                    <summary>{entry.tool.ok ? "View result" : "Error"}</summary>
                    <pre>{JSON.stringify(entry.tool.result, null, 2)}</pre>
                  </details>
                )}
              </article>
            ))
          : agent.changes.map((change) => (
              <article className="agent-message" key={change.sequence}>
                <strong>{change.title}</strong>
                <p>{change.detail}</p>
                {change.mutated && (
                  <button
                    className="m-button"
                    onClick={() =>
                      store.fire("web.agentRevert", {
                        sequence: change.sequence,
                        redo: !change.applied,
                      })
                    }
                  >
                    {change.applied ? "Revert" : "Redo"}
                  </button>
                )}
                <details>
                  <summary>Details</summary>
                  <pre>{change.output}</pre>
                </details>
              </article>
            ))}
        {!agent.transcript.entries.length && tab === "conversation" && (
          <p className="agent-empty">
            Describe what you want to make. The agent can inspect and edit this
            session.
          </p>
        )}
        <div ref={end} />
      </div>
      <div className="agent-composer">
        <div className="agent-status">
          {agent.status.status}
          {agent.status.running && (
            <button
              className="m-button"
              onClick={() => store.fire("agent.stop")}
            >
              Stop
            </button>
          )}
        </div>
        {agent.status.error && <p role="alert">{agent.status.error}</p>}
        <textarea
          aria-label="Message to agent"
          placeholder="Ask the agent…"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              send();
            }
          }}
        />
        <div className="agent-send">
          <span>⌘ Enter to send</span>
          <Button
            disabled={!draft.trim() || agent.status.running}
            onClick={send}
          >
            Send
          </Button>
        </div>
      </div>
    </aside>
  );
}
