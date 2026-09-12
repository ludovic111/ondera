import { beatsToBarBeat, beatsToSeconds, formatSmpte, secondsToSmpte } from '@ondera/core';
import { useSession } from '../../state/session';
import styles from './TimeDisplay.module.css';

const pad = (n: number, w: number) => String(n).padStart(w, '0');

/** Sunk LCD-style readout. Re-renders on every engine tick while playing. */
export function TimeDisplay() {
  const positionBeats = useSession((s) => s.transport.positionBeats);
  const tempo = useSession((s) => s.transport.tempo);
  const sig = useSession((s) => s.transport.timeSignature);
  const key = useSession((s) => s.transport.key);

  const pos = beatsToBarBeat(positionBeats, sig);
  const smpte = formatSmpte(secondsToSmpte(beatsToSeconds(positionBeats, tempo)));

  return (
    <div className={`${styles.display} m-well-deep`}>
      <div className={styles.cell}>
        <div className={styles.label}>Position</div>
        <div className={styles.position}>
          <span>{pad(pos.bar, 3)}</span>
          <span className={styles.dot}>·</span>
          <span>{pos.beat}</span>
          <span className={styles.dot}>·</span>
          <span>{pos.division}</span>
          <span className={styles.dot}>·</span>
          <span className={styles.ticks}>{pad(pos.tick, 3)}</span>
        </div>
      </div>
      <div className={styles.cell}>
        <div className={styles.label}>SMPTE</div>
        <div className={`${styles.value} ${styles.dim}`}>{smpte}</div>
      </div>
      <div className={styles.cell}>
        <div className={styles.label}>Tempo</div>
        <div className={styles.value}>{tempo.toFixed(2)}</div>
      </div>
      <div className={styles.cell}>
        <div className={styles.label}>Sig</div>
        <div className={styles.value}>
          {sig.numerator}/{sig.denominator}
        </div>
      </div>
      <div className={`${styles.cell} ${styles.last}`}>
        <div className={styles.label}>Key</div>
        <div className={styles.value}>{key}</div>
      </div>
    </div>
  );
}
