import type { ReactNode } from 'react';
import styles from './SegmentedControl.module.css';

export interface SegmentedItem<T extends string> {
  id: T;
  label: ReactNode;
  title?: string | undefined;
}

export interface SegmentedControlProps<T extends string> {
  items: readonly SegmentedItem<T>[];
  value: T;
  onChange?: (id: T) => void;
  /** Fill the container width. */
  stretch?: boolean;
  /** Square icon segments. */
  icons?: boolean;
}

/** A groove holding several segments; the active one is a raised slab. */
export function SegmentedControl<T extends string>({ items, value, onChange, stretch, icons }: SegmentedControlProps<T>) {
  return (
    <div className={[styles.group, 'm-groove-alt', stretch ? styles.stretch : '', icons ? styles.icons : ''].join(' ')}>
      {items.map((it) => (
        <div
          key={it.id}
          title={it.title}
          className={[styles.segment, it.id === value ? 'm-segment-active' : styles.inactive].join(' ')}
          onClick={onChange ? () => onChange(it.id) : undefined}
        >
          {it.label}
        </div>
      ))}
    </div>
  );
}
