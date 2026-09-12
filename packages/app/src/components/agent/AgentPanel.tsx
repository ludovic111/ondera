import { commands, type AgentLogEntry } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
import { Button } from '../primitives/Button';
import { CapsLabel } from '../primitives/CapsLabel';
import { ChevronRightIcon, SendIcon } from '../primitives/Icons';
import styles from './AgentPanel.module.css';

export function AgentPanel() {
  const dispatch = useDispatch();
  const agent = useSession((s) => s.agent);

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <span className={`${styles.dot} m-accent-dot`} />
        <span className={styles.title}>Agent</span>
        <span className={styles.status}>
          {agent.status} · {agent.transport}
        </span>
        <Button size="icon" className={styles.close} onClick={() => dispatch(commands.view.setAgentPanelOpen({ open: false }))}>
          <ChevronRightIcon />
        </Button>
      </div>

      {agent.current && (
        <div className={`${styles.card} m-glass-card`}>
          <CapsLabel style={{ color: 'var(--color-accent)', marginBottom: 6 }}>Now</CapsLabel>
          <div className={styles.cardText}>{agent.current.description}</div>
          <div className={styles.progressRow}>
            <div className={`${styles.progress} m-groove`}>
              <div className={styles.progressFill} style={{ width: `${Math.round(agent.current.progress * 100)}%` }} />
            </div>
            <span className={styles.progressLabel}>{agent.current.progressLabel}</span>
            <Button size="auto" className={styles.stop} onClick={() => dispatch(commands.agent.stopCurrent({}))}>
              Stop
            </Button>
          </div>
        </div>
      )}

      <div className={styles.log}>
        <div className={styles.logHead}>
          <CapsLabel>Changes this session</CapsLabel>
          <span className={styles.logCount}>{agent.log.length} entries</span>
        </div>
        <div className={styles.logList}>
          {agent.log.map((e) => (
            <LogRow key={e.id} entry={e} />
          ))}
        </div>
      </div>

      <div className={styles.inputArea}>
        <div className={styles.inputBox}>
          <input
            className={styles.input}
            value={agent.draft}
            onChange={(e) => dispatch(commands.agent.setDraft({ text: e.target.value }))}
            placeholder="Ask the agent… e.g. “double the chorus and mute BGV”"
          />
          <div className={styles.sendKey}>
            <SendIcon />
          </div>
        </div>
        <div className={styles.footer}>
          <span>⌘↵ send · ⌘Z reverts last agent change</span>
          <span>ondera-cli 0.9 · mcp</span>
        </div>
      </div>
    </div>
  );
}

function LogRow({ entry }: { entry: AgentLogEntry }) {
  const dispatch = useDispatch();
  return (
    <div className={`${styles.entry} ${entry.live ? styles.entryLive : ''} ${entry.reverted ? styles.entryReverted : ''}`}>
      <div className={`${styles.chip} ${entry.live ? styles.chipLive : ''}`}>
        <span className={`${styles.chipSwatch} m-swatch`} style={{ background: entry.color }} />
      </div>
      <div className={styles.entryText}>
        <div className={styles.entryTitle}>{entry.title}</div>
        <div className={styles.entryDetail}>{entry.detail}</div>
      </div>
      {entry.live ? (
        <Button size="auto" className={styles.entryButton} onClick={() => dispatch(commands.agent.stopCurrent({}))}>
          Stop
        </Button>
      ) : (
        <Button
          size="auto"
          className={styles.entryButton}
          pressed={entry.reverted}
          onClick={() => dispatch(commands.agent.toggleRevert({ entryId: entry.id }))}
        >
          {entry.reverted ? 'Redo' : 'Revert'}
        </Button>
      )}
    </div>
  );
}
