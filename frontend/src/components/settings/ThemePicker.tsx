import { useMemo } from "react";
import { TRACK_PALETTE } from "@ondera/core";
import { SegmentedControl } from "../primitives/SegmentedControl";
import {
  normalizeTheme,
  resolveMode,
  type ModeSetting,
} from "../../theme/applyTokens";
import { buildTheme } from "../../theme/tokens";
import { THEMES, THEME_NAMES, type ThemeId } from "../../theme/schema";
import styles from "./ThemePicker.module.css";

const BLURB: Record<ThemeId, string> = {
  modern: "Flat, quiet, one accent",
  skeuo: "Milled hardware, lit from above",
  aero: "Glass, water and sky",
  console: "Walnut, brass and amber lamps",
  ink: "Paper, ink and a red pencil",
  neon: "Violet glass lit from inside",
};
const CLIPS = [
  [TRACK_PALETTE.drums, 8, 46],
  [TRACK_PALETTE.bass, 8, 70],
  [TRACK_PALETTE.keys, 30, 52],
] as const;

/**
 * A window in miniature, painted with the real tokens of that theme. A theme that
 * dresses the window frame, the transport or the displays in its own stylesheet
 * names them `--<id>-frame`, `--<id>-bar` and `--<id>-lcd`.
 */
function Preview({ id, mode }: { id: ThemeId; mode: "dark" | "light" }) {
  const t = useMemo(() => buildTheme(id, mode), [id, mode]);
  const c = t.color;
  const own = (part: string): string | undefined => t.vars[`--${id}-${part}`];
  const frame = own("frame");
  return (
    <div
      className={styles.preview}
      style={{ background: frame ?? c.timeline, padding: frame ? 3 : 0 }}
    >
      <div
        className={styles.bar}
        style={{
          background: own("bar") ?? t.gradient.transport,
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
            background: own("lcd") ?? c.wellDeep,
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
      <div className={styles.lanes} style={{ background: c.timeline }}>
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

export function ThemePicker({
  appearance,
  mode,
  onAppearance,
  onMode,
}: {
  appearance: string;
  mode: string;
  onAppearance: (id: ThemeId) => void;
  onMode: (mode: ModeSetting) => void;
}) {
  const current = normalizeTheme(appearance);
  const setting = (
    ["dark", "light", "auto"].includes(mode) ? mode : "dark"
  ) as ModeSetting;
  const shown = resolveMode(setting);
  return (
    <div className={styles.picker}>
      <div className={styles.row}>
        <span>Theme</span>
        <SegmentedControl
          items={[
            { id: "dark", label: "Dark" },
            { id: "light", label: "Light" },
            { id: "auto", label: "Auto", title: "Follow the system" },
          ]}
          value={setting}
          onChange={onMode}
        />
      </div>
      <div className={styles.cards} role="radiogroup" aria-label="Theme">
        {THEMES.map((id) => (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={id === current}
            aria-pressed={id === current}
            className={styles.card}
            onClick={() => onAppearance(id)}
          >
            <Preview id={id} mode={shown} />
            <span className={styles.name}>{THEME_NAMES[id]}</span>
            <span className={styles.blurb}>{BLURB[id]}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
