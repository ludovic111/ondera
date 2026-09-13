import type { CSSProperties, MouseEvent, ReactNode } from "react";
import styles from "./Button.module.css";

export interface ButtonProps {
  children?: ReactNode;
  disabled?: boolean;
  title?: string | undefined;
  /** Sunk into the panel: the "on" state for toggles. */
  pressed?: boolean;
  /** Accent-lit: record arm and the agent send key only. */
  lit?: boolean;
  size?: "md" | "wide" | "sm" | "auto" | "icon";
  onClick?: (e: MouseEvent<HTMLElement>) => void;
  className?: string | undefined;
  style?: CSSProperties | undefined;
}

/** Raised by default; pressed when on; lit when armed. Same box in every state. */
export function Button({
  disabled,
  children,
  title,
  pressed,
  lit,
  size = "md",
  onClick,
  className,
  style,
}: ButtonProps) {
  const material = lit ? "m-lit" : pressed ? "m-pressed" : "m-raised";
  return (
    <button
      type="button"
      disabled={disabled}
      title={title}
      aria-label={title}
      aria-pressed={pressed ?? lit}
      onClick={onClick}
      className={[styles.button, styles[size], material, className]
        .filter(Boolean)
        .join(" ")}
      style={style}
    >
      {children}
    </button>
  );
}
