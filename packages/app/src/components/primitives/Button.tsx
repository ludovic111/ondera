import type { CSSProperties, ReactNode } from 'react';
import styles from './Button.module.css';

export interface ButtonProps {
  children?: ReactNode;
  title?: string | undefined;
  /** Sunk into the panel: the "on" state for toggles. */
  pressed?: boolean;
  /** Accent-lit: record arm and the agent send key only. */
  lit?: boolean;
  size?: 'md' | 'wide' | 'sm' | 'auto' | 'icon';
  onClick?: () => void;
  className?: string | undefined;
  style?: CSSProperties | undefined;
}

/** Raised by default; pressed when on; lit when armed. Same box in every state. */
export function Button({ children, title, pressed, lit, size = 'md', onClick, className, style }: ButtonProps) {
  const material = lit ? 'm-lit' : pressed ? 'm-pressed' : 'm-raised';
  return (
    <div
      role="button"
      title={title}
      onClick={onClick}
      className={[styles.button, styles[size], material, className].filter(Boolean).join(' ')}
      style={style}
    >
      {children}
    </div>
  );
}
