import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { isEnterKey } from '../../state/shortcuts';
import styles from './InlineEdit.module.css';

export interface InlineEditProps {
  value: string;
  onCommit: (value: string) => void;
  onCancel: () => void;
  className?: string | undefined;
  style?: CSSProperties | undefined;
  mono?: boolean;
}

/** A text field that appears in place of a label. Enter commits, Escape cancels, blur commits. */
export function InlineEdit({ value, onCommit, onCancel, className, style, mono }: InlineEditProps) {
  const [text, setText] = useState(value);
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  const commit = () => {
    if (done.current) return;
    done.current = true;
    const trimmed = text.trim();
    if (trimmed && trimmed !== value) onCommit(trimmed);
    else onCancel();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    e.stopPropagation();
    if (isEnterKey(e)) commit();
    if (e.key === 'Escape') {
      done.current = true;
      onCancel();
    }
  };

  return (
    <input
      ref={ref}
      className={[styles.input, mono ? 't-mono' : '', className].filter(Boolean).join(' ')}
      style={style}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={onKeyDown}
      onBlur={commit}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      spellCheck={false}
    />
  );
}
