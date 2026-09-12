import { commands } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
import styles from './AgentRail.module.css';

/** Collapsed agent panel: a 32 px rail with the status dot and a vertical label. */
export function AgentRail() {
  const dispatch = useDispatch();
  const status = useSession((s) => s.agent.status);
  return (
    <div className={styles.rail} onClick={() => dispatch(commands.view.setAgentPanelOpen({ open: true }))}>
      <span className={`${styles.dot} m-accent-dot`} />
      <span className={styles.label}>Agent · {status}</span>
    </div>
  );
}
