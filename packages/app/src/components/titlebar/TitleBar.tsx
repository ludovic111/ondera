import { useSession } from '../../state/session';
import styles from './TitleBar.module.css';

const MENU = ['File', 'Edit', 'Track', 'Mix', 'Agent', 'View', 'Help'];

/** Window chrome. Native traffic lights overlay the left inset on macOS. Menu labels are inert in Phase 1. */
export function TitleBar() {
  const name = useSession((s) => s.name);
  const isMac = (window.ondera?.platform ?? 'darwin') === 'darwin';
  return (
    <div className={`${styles.bar} ${isMac ? styles.mac : ''}`}>
      {!isMac && (
        <div className={styles.lights}>
          <span className={styles.light} />
          <span className={styles.light} />
          <span className={styles.light} />
        </div>
      )}
      <div className={styles.menu}>
        {MENU.map((m) => (
          <span key={m}>{m}</span>
        ))}
      </div>
      <div className={styles.title}>{name}</div>
    </div>
  );
}
