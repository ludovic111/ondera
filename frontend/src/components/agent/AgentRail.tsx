import { useSyncExternalStore } from "react";
import { commands } from "@ondera/core";
import { useDispatch, useStore } from "../../state/session";
import styles from "./AgentRail.module.css";

/** Collapsed agent panel: a 32 px rail with the status dot and a vertical label. */
export function AgentRail() {
  const dispatch = useDispatch();
  const store = useStore();
  const agent = useSyncExternalStore(store.subscribeMeta, store.getAgent);
  const status = agent.status.running ? "working" : "idle";
  return (
    <button
      aria-label="Open agent panel"
      className={styles.rail}
      onClick={() => dispatch(commands.view.setAgentPanelOpen({ open: true }))}
    >
      <span
        className={`${styles.dot} ${agent.status.running ? "m-accent-dot" : styles.idle}`}
      />
      <span className={styles.label}>Agent · {status}</span>
    </button>
  );
}
