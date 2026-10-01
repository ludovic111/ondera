import { useMemo } from "react";
import { TRACK_PALETTE } from "@ryolune/core";
import { resolveMode, type ModeSetting } from "../../theme/applyTokens";
import { buildTheme } from "../../theme/tokens";
import type { Mode } from "../../theme/schema";
import styles from "./ThemePicker.module.css";

const CHOICES: readonly { id: ModeSetting; name: string; blurb: string }[] = [
  { id: "dark", name: "Dark", blurb: "Graphite, for long sessions" },
  { id: "light", name: "Light", blurb: "Porcelain, for daylight" },
  { id: "auto", name: "Auto", blurb: "Follows the system" },
];
const CLIPS = [
  [TRACK_PALETTE.drums, 8, 46],
  [TRACK_PALETTE.bass, 8, 70],
  [TRACK_PALETTE.keys, 30, 52],
] as const;

/** A window in miniature, painted with the real tokens of one mode. */
function Preview({ mode }: { mode: Mode }) {
  const t = useMemo(() => buildTheme(mode), [mode]);
  const c = t.color;
  return (
    <div className={styles.preview} style={{ background: c.timeline }}>
      <div
        className={styles.bar}
        style={{
          background: t.gradient.transport,
          boxShadow: t.shadow.transport,
        }}
      >
        <span
          className={styles.key}
          style={{ background: t.gradient.lit, boxShadow: t.shadow.lit }}
        />
        <span
          className={styles.key}
          style={{
            background: t.gradient.raised,
            boxShadow: t.shadow.raisedSm,
          }}
        />
        <span
          className={styles.lcd}
          style={{
            background: t.vars["--ryo-display"] ?? c.wellDeep,
            color: c.wellInk,
            boxShadow: t.shadow.wellDeep,
          }}
        >
          004·2·1
        </span>
        <span
          className={styles.knob}
          style={{ background: t.gradient.knob, boxShadow: t.shadow.knob }}
        >
          <i style={{ background: c.indicator }} />
        </span>
      </div>
      <div className={styles.lanes}>
        {CLIPS.map(([track, left, width], i) => (
          <div
            key={i}
            className={styles.lane}
            style={{ boxShadow: `inset 0 -1px 0 ${t.line.laneBottom}` }}
          >
            <span
              className={styles.clip}
              style={{
                left: `${left}%`,
                width: `${width}%`,
                borderRadius: t.radius.clip / 2,
                background: `linear-gradient(color-mix(in oklab, ${track} ${t.clipMix.faceTop}%, ${c.panel}), color-mix(in oklab, ${track} ${t.clipMix.faceBottom}%, ${c.panel}))`,
                boxShadow: t.shadow.clip,
              }}
            />
          </div>
        ))}
        <span className={styles.playhead} style={{ background: c.accent }} />
      </div>
    </div>
  );
}

/** Dark, Light or Auto, each shown as the window it gives; Auto is split down the middle. */
export function ThemePicker({
  mode,
  onMode,
}: {
  mode: string;
  onMode: (mode: ModeSetting) => void;
}) {
  const setting = (
    ["dark", "light", "auto"].includes(mode) ? mode : "dark"
  ) as ModeSetting;
  return (
    <div className={styles.picker}>
      <span className={styles.label}>Appearance</span>
      <div className={styles.cards} role="radiogroup" aria-label="Appearance">
        {CHOICES.map(({ id, name, blurb }) => (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={id === setting}
            className={styles.card}
            onClick={() => onMode(id)}
          >
            {id === "auto" ? (
              <div className={styles.split}>
                <Preview mode="dark" />
                <Preview mode="light" />
              </div>
            ) : (
              <Preview mode={resolveMode(id)} />
            )}
            <span className={styles.name}>{name}</span>
            <span className={styles.blurb}>{blurb}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
