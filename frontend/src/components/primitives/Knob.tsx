import { useRef, type PointerEvent } from "react";
import styles from "./Knob.module.css";

export interface KnobProps {
  /** Rotation in degrees, 0 = straight up. */
  angle: number;
  size?: "sm" | "md" | "lg" | "xl";
  title?: string | undefined;
  /** Accessible name when it differs from the tooltip. */
  label?: string | undefined;
  /** Current value; with min/max and onChange the knob becomes draggable. */
  value?: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  /** Value restored on double-click. */
  defaultValue?: number;
  /** Draw the value ring; off where the surface draws its own (plugin faces). */
  ring?: boolean;
}

/** Pixels of vertical drag for the full value range. */
const DRAG_TRAVEL_PX = 160;
/** Diameter of each size in px (the size tokens), and the travel of the cap either side of up. */
const DIAMETER = { sm: 20, md: 26, lg: 36, xl: 46 } as const;
const SWEEP = 135;
/** Gap between the knob and its value ring, in px. */
const RING_GAP = 3;

/** An SVG arc on a circle of radius r around (c, c), clockwise from `from` to `to` degrees. */
function arc(c: number, r: number, from: number, to: number): string {
  const [a, b] = from <= to ? [from, to] : [to, from];
  const point = (deg: number) => {
    const t = (deg * Math.PI) / 180;
    return `${(c + r * Math.sin(t)).toFixed(2)} ${(c - r * Math.cos(t)).toFixed(2)}`;
  };
  return `M ${point(a)} A ${r} ${r} 0 ${b - a > 180 ? 1 : 0} 1 ${point(b)}`;
}

/**
 * A machined knob with a value ring. The body stays lit from above while the indicator
 * turns; the ring fills from the bottom-left stop, or from the top for a centred control
 * (pan, whose range runs below and above zero).
 * Drag up to increase, down to decrease; shift for fine control; double-click resets.
 */
export function Knob({
  angle,
  size = "sm",
  title,
  label,
  value,
  min,
  max,
  onChange,
  defaultValue,
  ring = true,
}: KnobProps) {
  const drag = useRef<{ y: number; value: number } | null>(null);
  const interactive =
    onChange !== undefined &&
    value !== undefined &&
    min !== undefined &&
    max !== undefined;

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!interactive) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { y: e.clientY, value: value! };
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (
      !interactive ||
      !drag.current ||
      !e.currentTarget.hasPointerCapture(e.pointerId)
    )
      return;
    const range = max! - min!;
    const fine = e.shiftKey ? 0.2 : 1;
    const next =
      drag.current.value +
      ((drag.current.y - e.clientY) / DRAG_TRAVEL_PX) * range * fine;
    onChange!(Math.min(max!, Math.max(min!, next)));
  };
  const onPointerUp = () => {
    drag.current = null;
  };
  const onDoubleClick = () => {
    if (interactive && defaultValue !== undefined) onChange!(defaultValue);
  };

  const bipolar = min !== undefined && max !== undefined && min < 0 && max > 0;
  const turn = Math.max(-SWEEP, Math.min(SWEEP, angle));
  const box = DIAMETER[size] + RING_GAP * 2 + 2;
  const centre = box / 2;
  const radius = DIAMETER[size] / 2 + RING_GAP;
  const from = bipolar ? 0 : -SWEEP;

  return (
    <div
      className={[
        styles.knob,
        styles[size],
        interactive ? styles.interactive : "",
      ].join(" ")}
      title={title}
      role={interactive ? "slider" : undefined}
      tabIndex={interactive ? 0 : undefined}
      aria-label={label ?? title ?? "Pan"}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      onKeyDown={(e) => {
        if (
          !interactive ||
          ![
            "ArrowUp",
            "ArrowRight",
            "ArrowDown",
            "ArrowLeft",
            "Home",
            "End",
          ].includes(e.key)
        )
          return;
        e.preventDefault();
        e.stopPropagation();
        const next =
          e.key === "Home"
            ? min!
            : e.key === "End"
              ? max!
              : value! +
                ((e.key === "ArrowUp" || e.key === "ArrowRight" ? 1 : -1) *
                  (max! - min!)) /
                  100;
        onChange!(Math.max(min!, Math.min(max!, next)));
      }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={onDoubleClick}
    >
      {ring && (
        <svg
          className={styles.ring}
          width={box}
          height={box}
          viewBox={`0 0 ${box} ${box}`}
          aria-hidden
        >
          <path
            className={styles.track}
            d={arc(centre, radius, -SWEEP, SWEEP)}
          />
          {Math.abs(turn - from) > 1 && (
            <path
              className={styles.value}
              d={arc(centre, radius, from, turn)}
            />
          )}
        </svg>
      )}
      {(size === "lg" || size === "xl") && <div className={styles.inner} />}
      <div className={styles.rotor} style={{ transform: `rotate(${turn}deg)` }}>
        <div className={styles.indicator} />
      </div>
    </div>
  );
}
