import styles from './LedStrip.module.css';

export interface LedStripProps {
  segments: number;
  /** 0..1 fraction lit. */
  level: number;
  /** Number of top segments that read as "hot" (accent) when lit. */
  hot?: number;
  orientation?: 'horizontal' | 'vertical';
  /** Segment thickness along the strip in px; only for horizontal strips. */
  segmentHeight?: number;
}

/** Discrete LED meter: warm-white segments in a dark well, accent for peaks. */
export function LedStrip({ segments, level, hot = 0, orientation = 'horizontal', segmentHeight }: LedStripProps) {
  const lit = Math.round(level * segments);
  const items = Array.from({ length: segments }, (_, i) => {
    const on = i < lit;
    const isHot = on && i >= segments - hot;
    const cls = isHot ? styles.hot : on ? styles.on : styles.off;
    return (
      <span
        key={i}
        className={`${styles.seg} ${cls}`}
        style={segmentHeight !== undefined ? { height: segmentHeight } : undefined}
      />
    );
  });
  return <div className={orientation === 'vertical' ? styles.vertical : styles.horizontal}>{items}</div>;
}
