import styles from './Knob.module.css';

export interface KnobProps {
  /** Rotation in degrees, 0 = straight up. */
  angle: number;
  size?: 'sm' | 'md' | 'lg';
  title?: string | undefined;
}

/** Milled aluminium knob. The whole cap rotates; the indicator is a slot. */
export function Knob({ angle, size = 'sm', title }: KnobProps) {
  return (
    <div className={[styles.knob, styles[size]].join(' ')} style={{ transform: `rotate(${angle}deg)` }} title={title}>
      {size === 'lg' && <div className={styles.inner} />}
      <div className={styles.indicator} />
    </div>
  );
}
