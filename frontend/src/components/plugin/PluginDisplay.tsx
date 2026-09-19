import { useId, type ReactNode } from "react";
import {
  compressDb,
  crush,
  envelopePoints,
  eqDb,
  filterDb,
  gateDb,
  hzToX,
  lfo,
  limitDb,
  plot,
  saturate,
  toneDb,
  xToHz,
} from "./response";
import styles from "./PluginFace.module.css";

export const DISPLAY_W = 560;
export const DISPLAY_H = 148;
const W = DISPLAY_W;
const H = DISPLAY_H;

export type Values = (name: string, fallback?: number) => number;

const FREQ_MARKS = [50, 100, 200, 500, 1000, 2000, 5000, 10000];
const hzLabel = (hz: number) => (hz >= 1000 ? `${hz / 1000}k` : String(hz));

/** Log-frequency graticule with dB rules; `range` is ± dB around the centre. */
function FrequencyGrid({ top, bottom }: { top: number; bottom: number }) {
  const rows: number[] = [];
  const step = top - bottom > 40 ? 12 : 6;
  for (let db = Math.ceil(bottom / step) * step; db <= top; db += step)
    rows.push(db);
  return (
    <g>
      {FREQ_MARKS.map((hz) => (
        <g key={hz}>
          <line
            x1={hzToX(hz) * W}
            x2={hzToX(hz) * W}
            y1={0}
            y2={H}
            className={styles.gridLine}
          />
          <text x={hzToX(hz) * W + 3} y={H - 4} className={styles.gridText}>
            {hzLabel(hz)}
          </text>
        </g>
      ))}
      {rows.map((db) => {
        const y = ((top - db) / (top - bottom)) * H;
        return (
          <g key={db}>
            <line
              x1={0}
              x2={W}
              y1={y}
              y2={y}
              className={db === 0 ? styles.zeroLine : styles.gridLine}
            />
            {db !== bottom && db !== top && (
              <text x={4} y={y - 3} className={styles.gridText}>
                {db > 0 ? `+${db}` : db}
              </text>
            )}
          </g>
        );
      })}
    </g>
  );
}

/** Square dB-in / dB-out graticule for the dynamics plugins. */
function LevelGrid({ floor }: { floor: number }) {
  const marks: number[] = [];
  for (let db = floor + 12; db < 0; db += 12) marks.push(db);
  return (
    <g>
      {marks.map((db) => {
        const t = (db - floor) / -floor;
        return (
          <g key={db}>
            <line
              x1={t * W}
              x2={t * W}
              y1={0}
              y2={H}
              className={styles.gridLine}
            />
            <line
              x1={0}
              x2={W}
              y1={(1 - t) * H}
              y2={(1 - t) * H}
              className={styles.gridLine}
            />
            <text x={t * W + 3} y={H - 4} className={styles.gridText}>
              {db}
            </text>
          </g>
        );
      })}
      <line x1={0} y1={H} x2={W} y2={0} className={styles.unityLine} />
    </g>
  );
}

function Curve({ d, fillTo }: { d: string; fillTo?: number }) {
  const id = useId();
  return (
    <g>
      {fillTo !== undefined && (
        <>
          <defs>
            <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" className={styles.fillTop} />
              <stop offset="1" className={styles.fillBottom} />
            </linearGradient>
          </defs>
          <path d={`${d}L${W} ${fillTo}L0 ${fillTo}Z`} fill={`url(#${id})`} />
        </>
      )}
      <path d={d} className={styles.curveGlow} />
      <path d={d} className={styles.curve} />
    </g>
  );
}

const Handle = ({ x, y }: { x: number; y: number }) => (
  <circle cx={x} cy={y} r={4.5} className={styles.handle} />
);

const Marker = ({ x, label }: { x: number; label: string }) => (
  <g>
    <line x1={x} x2={x} y1={0} y2={H} className={styles.marker} />
    <text
      x={Math.min(W - 4, x + 4)}
      y={12}
      className={styles.markerText}
      textAnchor={x > W - 60 ? "end" : "start"}
      dx={x > W - 60 ? -8 : 0}
    >
      {label}
    </text>
  </g>
);

function frequencyResponse(
  db: (hz: number) => number,
  top: number,
  bottom: number,
) {
  return plot((x) => (db(xToHz(x)) - bottom) / (top - bottom), W, H, 140);
}

function levelCurve(fn: (input: number) => number, floor: number) {
  return plot((x) => (fn(floor + x * -floor) - floor) / -floor, W, H, 120);
}

/** The picture for one stock plugin, or null when it has nothing to show. */
export function displayFor(name: string, v: Values): ReactNode {
  switch (name) {
    case "Channel EQ": {
      const bands = {
        lowGain: v("Low Gain"),
        lowFreq: v("Low Freq", 120),
        midGain: v("Mid Gain"),
        midFreq: v("Mid Freq", 1000),
        midQ: v("Mid Q", 0.8),
        highGain: v("High Gain"),
        highFreq: v("High Freq", 6000),
      };
      const y = (db: number) => ((18 - db) / 36) * H;
      return (
        <>
          <FrequencyGrid top={18} bottom={-18} />
          <Curve
            d={frequencyResponse((hz) => eqDb(bands, hz), 18, -18)}
            fillTo={y(0)}
          />
          <Handle
            x={hzToX(bands.lowFreq) * W}
            y={y(eqDb(bands, bands.lowFreq))}
          />
          <Handle
            x={hzToX(bands.midFreq) * W}
            y={y(eqDb(bands, bands.midFreq))}
          />
          <Handle
            x={hzToX(bands.highFreq) * W}
            y={y(eqDb(bands, bands.highFreq))}
          />
        </>
      );
    }
    case "Filter": {
      const cutoff = v("Cutoff", 1000);
      const db = (hz: number) =>
        filterDb(v("Type"), cutoff, v("Resonance"), hz);
      return (
        <>
          <FrequencyGrid top={24} bottom={-48} />
          <Curve d={frequencyResponse(db, 24, -48)} fillTo={H} />
          <Handle x={hzToX(cutoff) * W} y={((24 - db(cutoff)) / 72) * H} />
        </>
      );
    }
    case "Tape Sat":
    case "Overdrive": {
      const hard = name === "Overdrive";
      const drive = v("Drive");
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H / 2}
            y2={H / 2}
            className={styles.zeroLine}
          />
          <line
            x1={W / 2}
            x2={W / 2}
            y1={0}
            y2={H}
            className={styles.zeroLine}
          />
          <line x1={0} y1={H} x2={W} y2={0} className={styles.unityLine} />
          <Curve
            d={plot(
              (x) => 0.5 + 0.46 * saturate(x * 2 - 1, drive, hard),
              W,
              H,
              120,
            )}
          />
          <text
            x={W - 6}
            y={H - 6}
            textAnchor="end"
            className={styles.gridText}
          >
            tone {hzLabel(Math.round(v("Tone", 8000) / 100) * 100)} Hz ·{" "}
            {toneDb(v("Tone", 8000), 10000).toFixed(1)} dB @ 10k
          </text>
        </>
      );
    }
    case "Bitcrusher":
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H / 2}
            y2={H / 2}
            className={styles.zeroLine}
          />
          <path
            d={plot((x) => 0.5 + 0.44 * Math.sin(x * 4 * Math.PI), W, H, 120)}
            className={styles.ghost}
          />
          <Curve
            d={plot(
              (x) => 0.5 + 0.44 * crush(x, v("Bits", 8), v("Downsample", 4)),
              W,
              H,
              384,
            )}
          />
        </>
      );
    case "Ondera Comp": {
      const t = v("Threshold", -18);
      return (
        <>
          <LevelGrid floor={-60} />
          <Curve
            d={levelCurve(
              (i) => compressDb(i, t, v("Ratio", 4), v("Makeup")),
              -60,
            )}
            fillTo={H}
          />
          <Marker x={((t + 60) / 60) * W} label={`${t.toFixed(1)} dB`} />
        </>
      );
    }
    case "Gate": {
      const t = v("Threshold", -40);
      return (
        <>
          <LevelGrid floor={-80} />
          <Curve
            d={levelCurve((i) => gateDb(i, t, v("Range", -80)), -80)}
            fillTo={H}
          />
          <Marker x={((t + 80) / 80) * W} label={`${t.toFixed(1)} dB`} />
        </>
      );
    }
    case "Limiter": {
      const c = v("Ceiling", -0.3);
      return (
        <>
          <LevelGrid floor={-36} />
          <Curve
            d={levelCurve((i) => limitDb(i, v("Input"), c), -36)}
            fillTo={H}
          />
          <line
            x1={0}
            x2={W}
            y1={(-c / 36) * H}
            y2={(-c / 36) * H}
            className={styles.marker}
          />
        </>
      );
    }
    case "Auto Filter": {
      const cutoff = v("Cutoff", 600);
      const resonance = v("Resonance", 45);
      // The sweep the LFO and the envelope can reach, as ghosts either side.
      const reach =
        (v("LFO Depth", 40) / 100) * 3 +
        (Math.abs(v("Envelope", 40)) / 100) * 2;
      const at = (octaves: number) => (hz: number) =>
        filterDb(
          0,
          Math.min(18000, Math.max(30, cutoff * 2 ** octaves)),
          resonance,
          hz,
        );
      return (
        <>
          <FrequencyGrid top={24} bottom={-48} />
          {reach > 0.05 && (
            <>
              <path
                d={frequencyResponse(at(-reach), 24, -48)}
                className={styles.ghost}
              />
              <path
                d={frequencyResponse(at(reach), 24, -48)}
                className={styles.ghost}
              />
            </>
          )}
          <Curve d={frequencyResponse(at(0), 24, -48)} fillTo={H} />
          <Handle x={hzToX(cutoff) * W} y={((24 - at(0)(cutoff)) / 72) * H} />
        </>
      );
    }
    case "De-Esser": {
      const frequency = v("Frequency", 6500);
      const range = v("Range", 9);
      // The most it will take away: a high shelf of `range` dB above the split.
      const db = (hz: number) => {
        const f = (hz / (frequency * 0.8)) ** 2;
        return -range * (f / (1 + f));
      };
      return (
        <>
          <FrequencyGrid top={6} bottom={-24} />
          <Curve d={frequencyResponse(db, 6, -24)} fillTo={(6 / 30) * H} />
          <Handle x={hzToX(frequency) * W} y={((6 - db(frequency)) / 30) * H} />
        </>
      );
    }
    case "Lo-Fi": {
      const tone = v("Tone", 5200);
      const db = (hz: number) => 2 * toneDb(tone, hz);
      return (
        <>
          <FrequencyGrid top={12} bottom={-36} />
          <Curve d={frequencyResponse(db, 12, -36)} fillTo={H} />
          <Handle x={hzToX(tone) * W} y={((12 - db(tone)) / 48) * H} />
        </>
      );
    }
    case "Pitch Shift": {
      const shift = v("Semitones", 7) + v("Fine") / 100;
      // A 220 Hz note and where it lands, on the same log axis as the filters.
      const source = hzToX(220) * W;
      const target = hzToX(220 * 2 ** (shift / 12)) * W;
      return (
        <>
          <FrequencyGrid top={6} bottom={-6} />
          <line
            x1={source}
            x2={source}
            y1={H * 0.2}
            y2={H}
            className={styles.ghost}
          />
          <line
            x1={target}
            x2={target}
            y1={H * 0.2}
            y2={H}
            className={styles.marker}
          />
          <text x={target + 6} y={H * 0.2 + 10} className={styles.markerText}>
            {shift > 0 ? "+" : ""}
            {shift.toFixed(shift % 1 ? 2 : 0)} st
          </text>
        </>
      );
    }
    case "Pump": {
      const depth = v("Depth", 70) / 100;
      const recovery = Math.max(0.05, v("Recovery", 45) / 100);
      const offset = v("Offset") / 100;
      const gain = (x: number) => {
        const p = (((x * 4 - offset) % 1) + 1) % 1;
        const t = Math.min(1, p / recovery);
        return 1 - depth * (1 - t * t * (3 - 2 * t));
      };
      return (
        <>
          {[1, 2, 3].map((beat) => (
            <line
              key={beat}
              x1={(beat / 4) * W}
              x2={(beat / 4) * W}
              y1={0}
              y2={H}
              className={styles.gridLine}
            />
          ))}
          <Curve d={plot((x) => 0.06 + 0.88 * gain(x), W, H, 400)} fillTo={H} />
          <text
            x={W - 6}
            y={H - 6}
            textAnchor="end"
            className={styles.gridText}
          >
            4 pulses
          </text>
        </>
      );
    }
    case "Chorus":
    case "Phaser":
    case "Flanger":
    case "Auto Pan":
    case "Tremolo": {
      const depth = v("Depth", 50) / 100;
      // Two seconds of the modulator; the right channel is offset in stereo.
      const cycles = Math.max(0.25, Math.min(12, v("Rate", 1) * 2));
      const shape = name === "Tremolo" || name === "Auto Pan" ? v("Shape") : 0;
      const offset =
        name === "Tremolo"
          ? v("Stereo") / 360
          : name === "Chorus"
            ? (v("Spread", 50) / 100) * 0.25
            : name === "Auto Pan"
              ? 0.5
              : 0;
      const wave = (phase: number) => (x: number) =>
        0.5 + 0.44 * depth * lfo(shape, x * cycles + phase);
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H / 2}
            y2={H / 2}
            className={styles.zeroLine}
          />
          {offset > 0 && (
            <path d={plot(wave(offset), W, H, 240)} className={styles.ghost} />
          )}
          <Curve d={plot(wave(0), W, H, 240)} />
          <text
            x={W - 6}
            y={H - 6}
            textAnchor="end"
            className={styles.gridText}
          >
            2 s
          </text>
        </>
      );
    }
    case "Echo": {
      const feedback = v("Feedback", 35) / 100;
      const time = v("Time", 375);
      const ping = v("Ping-pong") >= 0.5;
      const taps: ReactNode[] = [];
      for (let i = 0; i < 14; i++) {
        const level = i === 0 ? 1 : Math.pow(feedback, i);
        const x = 14 + (i * time * (W - 28)) / Math.max(2400, time * 6);
        if (level < 0.015 || x > W - 8) break;
        const up = !ping || i % 2 === 0;
        taps.push(
          <rect
            key={i}
            x={x}
            y={up ? H / 2 - level * (H / 2 - 12) : H / 2}
            width={5}
            height={level * (H / 2 - 12)}
            rx={1.5}
            className={i === 0 ? styles.barDry : styles.bar}
          />,
        );
      }
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H / 2}
            y2={H / 2}
            className={styles.zeroLine}
          />
          {taps}
          {ping && (
            <>
              <text x={6} y={14} className={styles.gridText}>
                L
              </text>
              <text x={6} y={H - 6} className={styles.gridText}>
                R
              </text>
            </>
          )}
        </>
      );
    }
    case "Space": {
      const pre = v("Pre-delay", 10) / 100;
      const decay = 0.6 + (v("Size", 55) / 100) * 5;
      const damp = 1 + (v("Damp", 40) / 100) * 1.6;
      const start = 0.04 + pre * 0.2;
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H - 1}
            y2={H - 1}
            className={styles.zeroLine}
          />
          <rect
            x={10}
            y={10}
            width={4}
            height={H - 11}
            rx={1.5}
            className={styles.barDry}
          />
          <Curve
            d={plot(
              (x) =>
                x < start
                  ? 0
                  : 0.86 * Math.exp((-(x - start) * 6 * damp) / decay),
              W,
              H,
              160,
            )}
            fillTo={H}
          />
          <text x={W - 6} y={14} textAnchor="end" className={styles.gridText}>
            ≈ {decay.toFixed(1)} s
          </text>
        </>
      );
    }
    case "Stereo Width":
    case "Utility": {
      const width =
        name === "Stereo Width"
          ? v("Width", 100) / 100
          : v("Mono") >= 0.5
            ? 0
            : 1;
      const pan = name === "Utility" ? v("Pan") / 100 : 0;
      const cx = W / 2 + pan * (W / 2 - 80);
      const gain =
        name === "Utility" ? Math.pow(10, Math.min(12, v("Gain")) / 40) : 1;
      const ry = Math.min(H / 2 - 8, (H / 2 - 22) * gain);
      return (
        <>
          <line
            x1={W / 2}
            x2={W / 2}
            y1={0}
            y2={H}
            className={styles.zeroLine}
          />
          <line
            x1={0}
            x2={W}
            y1={H / 2}
            y2={H / 2}
            className={styles.gridLine}
          />
          <ellipse
            cx={cx}
            cy={H / 2}
            rx={Math.max(1.5, width * 110)}
            ry={ry}
            className={styles.field}
          />
          <text x={8} y={H / 2 - 5} className={styles.gridText}>
            L
          </text>
          <text
            x={W - 8}
            y={H / 2 - 5}
            textAnchor="end"
            className={styles.gridText}
          >
            R
          </text>
        </>
      );
    }
    case "Transient": {
      const attack = v("Attack") / 100;
      const sustain = v("Sustain") / 100;
      const shape = (a: number, s: number) => (x: number) => {
        const hit = Math.exp(-x * 22) * (0.62 + 0.36 * a);
        const body =
          Math.exp(-x * (3.2 - 2 * s)) * 0.42 * (1 - Math.exp(-x * 40));
        return Math.min(0.98, (hit + body) * Math.min(1, x * 90));
      };
      return (
        <>
          <line
            x1={0}
            x2={W}
            y1={H - 1}
            y2={H - 1}
            className={styles.zeroLine}
          />
          <path d={plot(shape(0, 0), W, H, 200)} className={styles.ghost} />
          <Curve d={plot(shape(attack, sustain), W, H, 200)} fillTo={H} />
        </>
      );
    }
    default:
      return null;
  }
}

/** Instruments show their amplitude envelope. */
export function envelopeDisplay(
  v: Values,
  has: (name: string) => boolean,
): ReactNode {
  if (!has("Attack") && !has("Release")) return null;
  const points = envelopePoints({
    attack: v("Attack", 5),
    decay: has("Decay") ? v("Decay", 200) : 1,
    sustain: has("Sustain") ? v("Sustain", 100) / 100 : 1,
    release: v("Release", 300),
  });
  const d = points
    .map(
      ([x, y], i) =>
        `${i ? "L" : "M"}${(8 + x * (W - 16)).toFixed(1)} ${(H - 8 - y * (H - 24)).toFixed(1)}`,
    )
    .join("");
  const labels = ["A", has("Decay") ? "D" : "", has("Sustain") ? "S" : "", "R"];
  return (
    <>
      <line x1={0} x2={W} y1={H - 8} y2={H - 8} className={styles.zeroLine} />
      <Curve d={d} fillTo={H - 8} />
      {points.slice(1).map(([x, y], i) => (
        <g key={i}>
          {i < 3 && <Handle x={8 + x * (W - 16)} y={H - 8 - y * (H - 24)} />}
          <text
            x={8 + ((points[i]![0] + x) / 2) * (W - 16)}
            y={14}
            textAnchor="middle"
            className={styles.gridText}
          >
            {labels[i]}
          </text>
        </g>
      ))}
    </>
  );
}
