import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useSession, useStore } from "../../state/session";
import { Button } from "../primitives/Button";
import {
  agentErrorMessage,
  canChat,
  providers,
  useAgentConnection,
} from "./connection";
import { RhythmLab } from "./RhythmLab";
import { TakePanel } from "./TakePanel";
import { slashCommands } from "./slashCommands";
import { AgentMessage, humanStatus } from "./AgentMessage";
import { ModelSelector } from "./ModelSelector";
import { selectionContext, splitContext } from "../../state/native";
import { describeTool, isReading, toolError, type ToolCall } from "./toolSteps";
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
  // Chips change only with the selection, not with every playhead tick.
  const chipKey = useSession((s) =>
    selectionContext(s)
      .map((c) => c.detail)
      .join("|"),
  );
  const chips = useMemo(() => selectionContext(store.getState()), [chipKey]);
  const {
    connection,
    checking,
    error: connectionError,
    check,
  } = useAgentConnection(`${settingsOpen}:${agent.status.provider}`);
  const [tab, setTab] = useState<
    "conversation" | "changes" | "takes" | "rhythm"
  >("conversation");
  const [slashIndex, setSlashIndex] = useState(0);
  const slashMatches =
    composer.draft.startsWith("/") && !composer.draft.includes(" ")
      ? slashCommands.filter((c) =>
          c.name.startsWith(composer.draft.slice(1).toLowerCase()),
        )
      : [];
  const chooseSlash = (command: (typeof slashCommands)[number]) => {
    if (command.name === "rhythm") {
      setTab("rhythm");
      store.setAgentDraft("");
    } else if (command.name === "takes" || command.name === "variation") {
      setTab("takes");
      store.setAgentDraft("");
    } else {
      store.setAgentDraft(command.prompt);
      input.current?.focus();
    }
    setSlashIndex(0);
  };
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
    if (tab === "conversation" && follow.current)
      end.current?.scrollIntoView({ block: "nearest" });
  }, [agent.transcript, tab]);
  useEffect(() => {
    if (tab !== "conversation" && conversation.current)
      conversation.current.scrollTop = 0;
  }, [tab]);
  // A context menu elsewhere asked for the agent: put the cursor in the message box.
  useEffect(() => {
    if (composer.focus > 0) {
      setTab("conversation");
      input.current?.focus();
    }
  }, [composer.focus]);
  const send = async () => {
    if (!ready || busy) return;
    follow.current = true;
    setTab("conversation");
    await store.sendAgent();
    input.current?.focus();
  };
  return (
    <aside className={styles.panel} aria-label="Agent" data-motion="from-right">
      <div className={styles.header} data-surface="agent-header">
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
        <button
          aria-pressed={tab === "takes"}
          className={tab === "takes" ? "m-segment-selected" : ""}
          onClick={() => setTab("takes")}
        >
          Takes A/B
        </button>
        <button
          aria-pressed={tab === "rhythm"}
          className={tab === "rhythm" ? "m-segment-selected" : ""}
          onClick={() => setTab("rhythm")}
        >
          Rhythm Lab
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
            {threadItems(agent.transcript.entries).map((item, index) =>
              item.kind === "steps" ? (
                <Steps key={index} steps={item.steps} />
              ) : (
                <article className="agent-message" key={index}>
                  <span className="caps">
                    {item.entry.role === "user" ? "You" : "Agent"}
                  </span>
                  {item.entry.role === "assistant" ? (
                    <AgentMessage
                      text={item.entry.text}
                      streaming={item.entry.streaming}
                    />
                  ) : (
                    <UserMessage text={item.entry.text} />
                  )}
                </article>
              ),
            )}
          </>
        ) : tab === "rhythm" ? (
          <RhythmLab busy={busy} />
        ) : tab === "takes" ? (
          <TakePanel
            busy={busy}
            onVariation={() => {
              setTab("conversation");
              store.setAgentDraft(
                "Explore a different musical direction in this creative take: [describe your idea]. The original version is preserved in Takes. Inspect the current project and preserve its identity.",
              );
              input.current?.focus();
            }}
          />
        ) : (
          <>
            {agent.transcript.entries
              .filter((entry) => entry.role === "notice")
              .map((entry, index) => (
                <article className="agent-message" key={`notice-${index}`}>
                  <strong>Details</strong>
                  <p>{entry.text}</p>
                </article>
              ))}
            {agent.changes.length === 0 ? (
              <div className={styles.welcome}>
                <h2>Your activity will appear here</h2>
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
                      store.fire("agent.revert", {
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
      <div
        className="agent-composer"
        hidden={tab === "rhythm" || tab === "takes"}
      >
        <ModelSelector
          key={`${agent.status.provider}:${agent.status.model}:${agent.status.reasoningEffort}`}
          provider={agent.status.provider}
          model={agent.status.model}
          effort={agent.status.reasoningEffort}
          disabled={busy}
        />
        <div className="agent-status" role="status">
          <span>
            {composer.sending
              ? "Sending…"
              : !ready
                ? "Connect a service to send"
                : humanStatus(
                    agent.status.status,
                    agent.status.running,
                    Boolean(error),
                  )}
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
            <button className="m-button" onClick={() => setTab("changes")}>
              View changes
            </button>
          </div>
        )}
        <div className={styles.context}>
          {chips.length === 0 ? (
            <span>The agent sees your whole project.</span>
          ) : (
            <>
              <button
                type="button"
                className={styles.contextToggle}
                aria-pressed={composer.withContext}
                title={
                  composer.withContext
                    ? "Your selection goes with the message. Click to send without it."
                    : "Click to send your selection with the message."
                }
                onClick={() => store.setAgentContext(!composer.withContext)}
              >
                {composer.withContext ? "About" : "Not about"}
              </button>
              {chips.map((chip) => (
                <span
                  key={chip.label}
                  className={`${styles.subject} ${composer.withContext ? "" : styles.subjectOff}`}
                  title={chip.detail}
                >
                  {chip.label}
                </span>
              ))}
            </>
          )}
        </div>
        {slashMatches.length > 0 && (
          <div
            className={styles.slashMenu}
            role="listbox"
            id="agent-slash-menu"
            aria-label="Agent commands"
          >
            {slashMatches.map((command, index) => (
              <button
                type="button"
                role="option"
                aria-selected={index === slashIndex}
                id={`agent-slash-${index}`}
                key={command.name}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => chooseSlash(command)}
              >
                <strong>/{command.name}</strong>
                <span>{command.label}</span>
              </button>
            ))}
          </div>
        )}
        <textarea
          ref={input}
          aria-label="Message to agent"
          aria-describedby="agent-send-help"
          placeholder="Describe your idea, or type / for commands…"
          aria-controls={slashMatches.length ? "agent-slash-menu" : undefined}
          aria-activedescendant={
            slashMatches.length ? `agent-slash-${slashIndex}` : undefined
          }
          value={composer.draft}
          onChange={(e) => {
            store.setAgentDraft(e.target.value);
            setSlashIndex(0);
          }}
          onKeyDown={(e) => {
            if (slashMatches.length && !e.nativeEvent.isComposing) {
              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault();
                setSlashIndex(
                  (i) =>
                    (i +
                      (e.key === "ArrowDown" ? 1 : -1) +
                      slashMatches.length) %
                    slashMatches.length,
                );
                return;
              }
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                chooseSlash(slashMatches[slashIndex] ?? slashMatches[0]);
                return;
              }
              if (e.key === "Escape") {
                e.preventDefault();
                store.setAgentDraft("");
                return;
              }
            }
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

type Entry = {
  role: string;
  text: string;
  streaming?: boolean;
  tool?: ToolCall;
};
type Item =
  { kind: "message"; entry: Entry } | { kind: "steps"; steps: ToolCall[] };

/** Messages in order, with each run of tool calls folded into one block of steps. */
function threadItems(entries: Entry[]): Item[] {
  const items: Item[] = [];
  for (const entry of entries) {
    if (entry.role === "tool" && entry.tool) {
      const last = items[items.length - 1];
      if (last?.kind === "steps") last.steps.push(entry.tool);
      else items.push({ kind: "steps", steps: [entry.tool] });
    } else if (entry.role === "user" || entry.role === "assistant") {
      items.push({ kind: "message", entry });
    }
  }
  return items;
}

/** What the person typed, with the selection that went along shown as a quiet caption. */
function UserMessage({ text }: { text: string }) {
  const { text: words, context } = splitContext(text);
  return (
    <div>
      {words}
      {context && (
        <div className={styles.sentContext} title={context}>
          about {context.replace(/ \((?:clipId|trackId)[^)]*\)/g, "")}
        </div>
      )}
    </div>
  );
}

/**
 * What the agent did between two messages. Edits are listed; a run of pure reading
 * collapses to one line, because "looked at the project" six times is noise.
 */
function Steps({ steps }: { steps: ToolCall[] }) {
  const edits = steps.filter((s) => !isReading(s) || !s.ok);
  const reads = steps.length - edits.length;
  return (
    <div
      className={styles.steps}
      aria-label="What the agent did"
      data-motion="reveal"
    >
      {reads > 0 && (
        <div className={styles.stepQuiet}>
          Looked at your project{reads > 1 ? ` · ${reads} checks` : ""}
        </div>
      )}
      {edits.map((step, index) => (
        <details key={index} className={styles.step}>
          <summary>
            <span
              className={`${styles.stepMark} ${step.ok ? "" : styles.stepFailed}`}
              aria-hidden
            >
              {step.ok ? "✓" : "!"}
            </span>
            <span className={styles.stepLabel}>{describeTool(step)}</span>
            {!step.ok && (
              <span className={styles.stepWhy}>{toolError(step)}</span>
            )}
          </summary>
          <pre>
            {step.name.replace("_", ".")}{" "}
            {JSON.stringify(step.args ?? {}, null, 1)}
          </pre>
        </details>
      ))}
    </div>
  );
}
