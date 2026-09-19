import type { CSSProperties, ReactNode } from "react";
import { Knob } from "../primitives/Knob";
import { SegmentedControl } from "../primitives/SegmentedControl";
import {
  DISPLAY_H,
  DISPLAY_W,
  displayFor,
  envelopeDisplay,
} from "./PluginDisplay";
import styles from "./PluginFace.module.css";

export interface Parameter {
  id: number;
  name: string;
  min: number;
  max: number;
  value: number;
  default: number;
  unit: string;
  steps: number;
  labels: string[];
  logarithmic: boolean;
}

/** Knob travel: 270°, from 7 o'clock to 5 o'clock. */
const SWEEP = 270;
const RING = 60;
const R = 26;

/** Parameter value ↔ knob position 0..1, logarithmic where the plugin says so. */
export function toPosition(p: Parameter, value: number): number {
  if (p.max === p.min) return 0;
  const v = Math.min(p.max, Math.max(p.min, value));
  return p.logarithmic && p.min > 0
    ? Math.log(v / p.min) / Math.log(p.max / p.min)
    : (v - p.min) / (p.max - p.min);
}
export function fromPosition(p: Parameter, position: number): number {
  const t = Math.min(1, Math.max(0, position));
  return p.logarithmic && p.min > 0
    ? p.min * Math.pow(p.max / p.min, t)
    : p.min + t * (p.max - p.min);
}

export function formatValue(p: Parameter, value: number): string {
  if (p.labels.length) return p.labels[Math.round(value)] ?? "";
  if (p.unit === "Hz" && value >= 1000)
    return `${(value / 1000).toFixed(value >= 10000 ? 1 : 2)} kHz`;
  const abs = Math.abs(value);
  const digits = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  const text = value.toFixed(digits);
  const space = p.unit === ":1" || p.unit === "°" || p.unit === "x" ? "" : " ";
  return p.unit ? `${text}${space}${p.unit}` : text;
}

const polar = (t: number): [number, number] => {
  const a = ((t * SWEEP - SWEEP / 2 - 90) * Math.PI) / 180;
  return [RING / 2 + R * Math.cos(a), RING / 2 + R * Math.sin(a)];
};
function arc(from: number, to: number): string {
  const [a, b] = from <= to ? [from, to] : [to, from];
  const [x1, y1] = polar(a);
  const [x2, y2] = polar(b);
  const large = (b - a) * SWEEP > 180 ? 1 : 0;
  return `M${x1.toFixed(2)} ${y1.toFixed(2)}A${R} ${R} 0 ${large} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
}

function Dial({
  p,
  onChange,
  onAutomate,
}: {
  p: Parameter;
  onChange: (value: number) => void;
  onAutomate: () => void;
}) {
  const position = toPosition(p, p.value);
  // A range that straddles zero fills from its centre, like a pan or a gain.
  const origin = p.min < 0 && p.max > 0 ? toPosition(p, 0) : 0;
  return (
    <div className={styles.control}>
      <span className={`${styles.label} t-caps`}>{p.name}</span>
      <div className={styles.dial}>
        <svg width={RING} height={RING} className={styles.ring} aria-hidden>
          <path d={arc(0, 1)} className={styles.ringTrack} />
          {Math.abs(position - origin) > 0.004 && (
            <path d={arc(origin, position)} className={styles.ringValue} />
          )}
        </svg>
        <Knob
          size="xl"
          angle={position * SWEEP - SWEEP / 2}
          label={p.name}
          title={`${p.name}: drag, Shift for fine, double-click to reset`}
          value={position}
          min={0}
          max={1}
          defaultValue={toPosition(p, p.default)}
          onChange={(t) => onChange(fromPosition(p, t))}
        />
      </div>
      <span className={`${styles.value} t-mono`}>
        {formatValue(p, p.value)}
      </span>
      <button
        type="button"
        className={styles.automate}
        title={`Automate ${p.name}`}
        aria-label={`Automate ${p.name}`}
        onClick={onAutomate}
      >
        A
      </button>
    </div>
  );
}

function Choice({
  p,
  onChange,
}: {
  p: Parameter;
  onChange: (value: number) => void;
}) {
  const index = Math.round(p.value);
  const onOff = p.labels.length === 2 && p.labels[0] === "Off";
  return (
    <div className={styles.choice}>
      <span className={`${styles.label} t-caps`}>{p.name}</span>
      {onOff ? (
        <button
          type="button"
          role="switch"
          aria-checked={index === 1}
          aria-label={p.name}
          className={`${styles.switch} ${index === 1 ? "m-lit" : "m-raised-sm"}`}
          onClick={() => onChange(index === 1 ? 0 : 1)}
        >
          {index === 1 ? "On" : "Off"}
        </button>
      ) : p.labels.length <= 4 ? (
        <SegmentedControl
          items={p.labels.map((label, i) => ({ id: String(i), label }))}
          value={String(index)}
          onChange={(id) => onChange(Number(id))}
        />
      ) : (
        <select
          aria-label={p.name}
          value={index}
          onChange={(e) => onChange(Number(e.target.value))}
        >
          {p.labels.map((label, i) => (
            <option key={label} value={i}>
              {label}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

export interface PluginFaceProps {
  name: string;
  category: string;
  /** CSS colour of the plugin's sound family; tints the trim, dials and display. */
  tint?: string | undefined;
  parameters: Parameter[];
  onChange: (p: Parameter, value: number) => void;
  onAutomate: (p: Parameter) => void;
  /** Preset controls, placed at the right of the nameplate. */
  toolbar: ReactNode;
}

/**
 * The front panel of a stock plugin: nameplate, a display that draws what the
 * plugin is doing, then one dial per continuous parameter and one selector per
 * choice. Built only from the material classes, so it follows the theme.
 */
export function PluginFace({
  name,
  category,
  tint,
  parameters,
  onChange,
  onAutomate,
  toolbar,
}: PluginFaceProps) {
  const byName = new Map(parameters.map((p) => [p.name, p]));
  const value = (key: string, fallback = 0) =>
    byName.get(key)?.value ?? fallback;
  const display =
    displayFor(name, value) ?? envelopeDisplay(value, (key) => byName.has(key));
  const choices = parameters.filter((p) => p.labels.length > 0);
  const dials = parameters.filter((p) => p.labels.length === 0);
  return (
    <div
      className={styles.face}
      style={tint ? ({ "--family": tint } as CSSProperties) : undefined}
    >
      <div className={styles.nameplate}>
        <div className={styles.identity}>
          <span className={tint ? styles.led : `${styles.led} m-accent-dot`} />
          <div>
            <div className={styles.name}>{name}</div>
            <div className={`${styles.category} t-caps`}>{category}</div>
          </div>
        </div>
        <div className={styles.toolbar}>{toolbar}</div>
      </div>
      {display && (
        <div className={`${styles.display} m-well-deep`}>
          <svg
            viewBox={`0 0 ${DISPLAY_W} ${DISPLAY_H}`}
            preserveAspectRatio="none"
            role="img"
            aria-label={`${name} response`}
          >
            {display}
          </svg>
        </div>
      )}
      {choices.length > 0 && (
        <div className={styles.choices}>
          {choices.map((p) => (
            <Choice key={p.id} p={p} onChange={(v) => onChange(p, v)} />
          ))}
        </div>
      )}
      {dials.length > 0 && (
        <div className={`${styles.dials} m-plate`}>
          {dials.map((p) => (
            <Dial
              key={p.id}
              p={p}
              onChange={(v) => onChange(p, v)}
              onAutomate={() => onAutomate(p)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
