import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useSession, useStore } from "../../state/session";
import { Button } from "../primitives/Button";
import {
  agentErrorMessage,
  canChat,
  providers,
  useAgentConnection,
} from "./connection";
import styles from "./AgentPanel.module.css";

const starters = [
  [
    "Start a beat",
    "Add a four-bar drum groove at the current tempo. Keep the rest of my project.",
  ],
  [
    "Write some chords",
    "Add a warm four-bar chord progression that fits this project, on a new instrument track.",
  ],
  [
    "Help with my mix",
    "Inspect my mix and suggest three improvements. Explain them before changing anything.",
  ],
];

export function AgentPanel() {
  const store = useStore();
  const agent = useSyncExternalStore(store.subscribeMeta, store.getAgent);
  const composer = useSyncExternalStore(store.subscribeMeta, store.getComposer);
  const settingsOpen = useSyncExternalStore(
    store.subscribeMeta,
    () => store.ui.settings,
  );
  const selectedTrack = useSession((s) =>
    s.tracks.find((track) => track.id === s.view.selectedTrackId),
  );
  const {
    connection,
    checking,
    error: connectionError,
    check,
  } = useAgentConnection(`${settingsOpen}:${agent.status.provider}`);
  const [tab, setTab] = useState<"conversation" | "changes">("conversation");
  const [confirmClear, setConfirmClear] = useState(false);
  const end = useRef<HTMLDivElement>(null);
  const conversation = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const input = useRef<HTMLTextAreaElement>(null);
  const error = composer.error || agent.status.error;
  const lastRequest = [...agent.transcript.entries]
    .reverse()
    .find((entry) => entry.role === "user")?.text;
  const busy = agent.status.running || composer.sending;
  const ready = canChat(connection);
  const openSettings = () =>
    store.fire("ui.showPanel", { panel: "settings", section: "agent" });
  useEffect(() => {
    if (follow.current) end.current?.scrollIntoView({ block: "nearest" });
  }, [agent.transcript, tab]);
  const send = async () => {
    if (!ready || busy) return;
    follow.current = true;
    setTab("conversation");
    await store.sendAgent();
    input.current?.focus();
  };
  return (
    <aside className={styles.panel} aria-label="Agent">
      <div className={styles.header}>
        <span className={`${styles.dot} ${ready ? "m-led-accent" : ""}`} />
        <strong className={styles.title}>Agent</strong>
        <span className={styles.status}>
          {connection ? providers[connection.provider].name : ""}
        </span>
        <Button size="icon" onClick={openSettings} title="Agent settings">
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
      <div className="agent-tabs" aria-label="Agent views">
        <button
          aria-pressed={tab === "conversation"}
          className={tab === "conversation" ? "m-segment-selected" : ""}
          onClick={() => {
            follow.current = true;
            setTab("conversation");
          }}
        >
          Conversation
        </button>
        <button
          aria-pressed={tab === "changes"}
          className={tab === "changes" ? "m-segment-selected" : ""}
          onClick={() => setTab("changes")}
        >
          Changes · {agent.changes.length}
        </button>
      </div>
      <div
        className="agent-conversation"
        ref={conversation}
        onScroll={() => {
          const element = conversation.current;
          if (element)
            follow.current =
              element.scrollHeight - element.scrollTop - element.clientHeight <
              80;
        }}
      >
        {tab === "conversation" ? (
          <>
            {!ready && (
              <div className={styles.welcome}>
                <h2>
                  {checking
                    ? "Checking your agent…"
                    : "Connect your music assistant"}
                </h2>
                <p>
                  {connectionError ||
                    connection?.message ||
                    "Choose an AI service and connect your account. Then describe your ideas in your own words."}
                </p>
                {!checking && (
                  <div className={styles.welcomeActions}>
                    <Button onClick={openSettings}>Set up agent</Button>
                    <Button onClick={() => void check()}>Check again</Button>
                  </div>
                )}
              </div>
            )}
            {!agent.transcript.entries.length && (
              <div className={styles.welcome}>
                <h2>
                  {ready
                    ? "What would you like to make?"
                    : "A few ideas to get started"}
                </h2>
                <p>
                  Ask for a beat, a melody or help with your mix. You can write
                  in your own language.
                </p>
                <div className={styles.starters}>
                  {starters.map(([title, prompt]) => (
                    <button
                      className="m-button"
                      key={title}
                      disabled={busy}
                      onClick={() => {
                        store.setAgentDraft(prompt);
                        input.current?.focus();
                      }}
                    >
                      {title}
                      <span>↗</span>
                    </button>
                  ))}
                </div>
                <p className={styles.hint}>
                  Suggestions fill your message. You choose when to send it.
                </p>
              </div>
            )}
            {agent.transcript.entries.map((entry, index) => (
              <article className="agent-message" key={index}>
                <span className="caps">
                  {entry.role === "user"
                    ? "You"
                    : entry.role === "assistant"
                      ? "Agent"
                      : (entry.tool?.name ?? "Notice")}
                </span>
                <div>{entry.text}</div>
                {entry.tool && (
                  <details>
                    <summary>
                      {entry.tool.result == null
                        ? "Working…"
                        : entry.tool.ok
                          ? "View result"
                          : "Could not complete this step"}
                    </summary>
                    <pre>{JSON.stringify(entry.tool.result, null, 2)}</pre>
                  </details>
                )}
              </article>
            ))}
          </>
        ) : (
          <>
            {agent.changes.length === 0 ? (
              <div className={styles.welcome}>
                <h2>Your changes will appear here</h2>
                <p>
                  When the agent edits your project, you can review the result
                  and undo it here.
                </p>
              </div>
            ) : (
              <p className={styles.historyNote}>
                Undo from here also undoes later edits, including yours. Stop
                the agent before reviewing changes.
              </p>
            )}
            {agent.changes.map((change) => (
              <article className="agent-message" key={change.sequence}>
                <strong>{change.title}</strong>
                <p>{change.detail}</p>
                {change.mutated && (
                  <button
                    className="m-button"
                    disabled={busy}
                    title={
                      change.applied
                        ? "Undo this change and all later edits"
                        : "Redo through this change"
                    }
                    onClick={() =>
                      store.fire("web.agentRevert", {
                        sequence: change.sequence,
                        redo: !change.applied,
                      })
                    }
                  >
                    {change.applied ? "Undo from here" : "Redo to here"}
                  </button>
                )}
                <details>
                  <summary>
                    {change.succeeded ? "Details" : "Error details"}
                  </summary>
                  <pre>{change.output}</pre>
                </details>
              </article>
            ))}
          </>
        )}
        <div ref={end} />
      </div>
      <div className="agent-composer">
        <div className="agent-status" role="status">
          <span>
            {composer.sending
              ? "Sending…"
              : agent.status.status ||
                (ready ? "Ready when you are" : "Connect a service to send")}
          </span>
          {agent.status.running && (
            <button
              className="m-button"
              onClick={() => store.fire("agent.stop")}
            >
              Stop
            </button>
          )}
        </div>
        {error && (
          <div className={styles.error} role="alert">
            <p>{agentErrorMessage(error, Boolean(composer.error))}</p>
            <button className="m-button" onClick={openSettings}>
              Check agent settings
            </button>
            {!composer.draft && lastRequest && !busy && (
              <button
                className="m-button"
                onClick={() => {
                  store.setAgentDraft(lastRequest);
                  input.current?.focus();
                }}
              >
                Edit last request
              </button>
            )}
            <details>
              <summary>Technical details</summary>
              <p>{error}</p>
            </details>
          </div>
        )}
        <div className={styles.context} title={selectedTrack?.name}>
          Context:{" "}
          {selectedTrack
            ? `“${selectedTrack.name}” selected · current project`
            : "Current project"}
        </div>
        <textarea
          ref={input}
          aria-label="Message to agent"
          aria-describedby="agent-send-help"
          placeholder="Describe the music you want to make…"
          value={composer.draft}
          onChange={(e) => store.setAgentDraft(e.target.value)}
          onKeyDown={(e) => {
            if (
              e.key === "Enter" &&
              !e.shiftKey &&
              !e.nativeEvent.isComposing &&
              !e.repeat
            ) {
              e.preventDefault();
              void send();
            }
          }}
        />
        <div className="agent-send">
          <span id="agent-send-help">
            Enter to send · Shift Enter for a new line
          </span>
          <Button
            disabled={!composer.draft.trim() || busy || !ready || checking}
            onClick={() => void send()}
          >
            Send
          </Button>
        </div>
        {agent.transcript.entries.length > 0 && (
          <div className={styles.newConversation}>
            {confirmClear ? (
              <>
                <span>Clear the conversation? Your music stays.</span>
                <button
                  className="m-button"
                  onClick={() => setConfirmClear(false)}
                >
                  Cancel
                </button>
                <button
                  className="m-button"
                  disabled={busy}
                  onClick={() => {
                    void store
                      .run("agent.clear")
                      .then(() => setConfirmClear(false))
                      .catch(() => {});
                  }}
                >
                  Clear conversation
                </button>
              </>
            ) : (
              <button
                className="m-button"
                disabled={busy}
                onClick={() => setConfirmClear(true)}
              >
                New conversation
              </button>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}
