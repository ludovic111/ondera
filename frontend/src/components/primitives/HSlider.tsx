import { useRef, type PointerEvent } from "react";
import styles from "./HSlider.module.css";

export interface HSliderProps {
  /** 0..1 */
  value: number;
  onChange?: (value: number) => void;
  title?: string | undefined;
}

/** Horizontal fader: groove rail with a milled square thumb. */
export function HSlider({ value, onChange, title }: HSliderProps) {
  const railRef = useRef<HTMLDivElement>(null);

  const valueFromEvent = (e: PointerEvent) => {
    const rail = railRef.current;
    if (!rail) return value;
    const rect = rail.getBoundingClientRect();
    const thumb = rect.height + 9; // thumb is 13px on a 4px rail
    const travel = rect.width - thumb;
    return Math.min(
      1,
      Math.max(0, (e.clientX - rect.left - thumb / 2) / travel),
    );
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!onChange) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    onChange(valueFromEvent(e));
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!onChange || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
    onChange(valueFromEvent(e));
  };

  return (
    <div
      ref={railRef}
      className={`${styles.rail} m-groove`}
      title={title}
      role="slider"
      tabIndex={0}
      aria-label={title ?? "Volume"}
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={value}
      onKeyDown={(e) => {
        if (
          [
            "ArrowUp",
            "ArrowRight",
            "ArrowDown",
            "ArrowLeft",
            "Home",
            "End",
          ].includes(e.key)
        ) {
          e.preventDefault();
          e.stopPropagation();
          onChange?.(
            e.key === "Home"
              ? 0
              : e.key === "End"
                ? 1
                : Math.max(
                    0,
                    Math.min(
                      1,
                      value +
                        (e.key === "ArrowUp" || e.key === "ArrowRight"
                          ? 0.01
                          : -0.01),
                    ),
                  ),
          );
        }
      }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
    >
      <div
        className={styles.thumb}
        style={{ left: `calc(${value} * (100% - var(--size-slider-thumb)))` }}
      >
        <div className={styles.milled} />
      </div>
    </div>
  );
}
