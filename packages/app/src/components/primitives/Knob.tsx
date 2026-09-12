import { useRef, type PointerEvent } from 'react';
import styles from './Knob.module.css';

export interface KnobProps {
  /** Rotation in degrees, 0 = straight up. */
  angle: number;
  size?: 'sm' | 'md' | 'lg';
  title?: string | undefined;
  /** Current value; with min/max and onChange the knob becomes draggable. */
  value?: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  /** Value restored on double-click. */
  defaultValue?: number;
}

/** Pixels of vertical drag for the full value range. */
const DRAG_TRAVEL_PX = 160;

/**
 * Milled aluminium knob. The whole cap rotates; the indicator is a slot.
 * Drag up to increase, down to decrease; shift for fine control; double-click resets.
 */
export function Knob({ angle, size = 'sm', title, value, min, max, onChange, defaultValue }: KnobProps) {
  const drag = useRef<{ y: number; value: number } | null>(null);
  const interactive = onChange !== undefined && value !== undefined && min !== undefined && max !== undefined;

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!interactive) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { y: e.clientY, value: value! };
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!interactive || !drag.current || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const range = max! - min!;
    const fine = e.shiftKey ? 0.2 : 1;
    const next = drag.current.value + ((drag.current.y - e.clientY) / DRAG_TRAVEL_PX) * range * fine;
    onChange!(Math.min(max!, Math.max(min!, next)));
  };
  const onPointerUp = () => {
    drag.current = null;
  };
  const onDoubleClick = () => {
    if (interactive && defaultValue !== undefined) onChange!(defaultValue);
  };

  return (
    <div
      className={[styles.knob, styles[size], interactive ? styles.interactive : ''].join(' ')}
      style={{ transform: `rotate(${angle}deg)` }}
      title={title}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={onDoubleClick}
    >
      {size === 'lg' && <div className={styles.inner} />}
      <div className={styles.indicator} />
    </div>
  );
}
