import {
  useState,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  clipEnvelope,
  CLIP_GAIN_MAX_DB,
  CLIP_GAIN_MIN_DB,
  commands,
  dbToFader,
  FADE_CURVE_LABELS,
  FADE_CURVES,
  type AudioClipData,
  type FadeCurve,
  defaultStrip,
  faderToDb,
  formatDb,
  MAX_SENDS,
  outputName,
  type InsertSlot,
  type Send,
  type Track,
} from "@ryolune/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { Knob } from "../primitives/Knob";
import { RecordSmallIcon } from "../primitives/Icons";
import { LedStrip } from "../primitives/LedStrip";
import { CapsLabel } from "../primitives/CapsLabel";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import { separator } from "../../state/menus";
import { size } from "../../theme/tokens";
import styles from "./InspectorPanel.module.css";

const PAN_DEGREES = 1.35;
const CHANNEL_SEGMENTS = 20;
const SEND_MIN_DB = -48;

const formatPan = (pan: number) =>
  pan === 0 ? "C" : pan < 0 ? `L ${-Math.round(pan)}` : `R ${Math.round(pan)}`;
const sendAngle = (db: number) =>
  db === -Infinity
    ? -135
    : Math.max(
        -135,
        Math.min(135, -135 + ((db - SEND_MIN_DB) / -SEND_MIN_DB) * 270),
      );
const faderTravel = size.faderH - size.faderCapH;
const faderCapTop = (position: number) => (1 - position) * faderTravel;

export function InspectorPanel() {
  const store = useStore();
  const dispatch = useDispatch();
  const selected = useSession((s) => s.view.selectedTrackId);
  const masterVolume = useSession(() => store.document?.masterVolume ?? 0.75);
  const actualTrack = useSession(
    (s) => s.tracks.find((t) => t.id === s.view.selectedTrackId) ?? null,
  );
  const isBus =
    selected === "master" || selected === "bus-a" || selected === "bus-b";
  const track: Track | null =
    actualTrack ??
    (isBus
      ? {
          id: selected!,
          name:
            selected === "master"
              ? "Stereo Out"
              : selected === "bus-a"
                ? "Reverb (A)"
                : "Delay (B)",
          kind: "audio",
          color: "var(--color-accent)",
          volume: masterVolume,
          pan: 0,
          mute: false,
          solo: false,
          armed: false,
          agentActive: false,
        }
      : null);
  const stored = useSession((s) => (track ? s.strips[track.id] : undefined));
  const tracks = useSession((s) => s.tracks);
  const meters = useSession((s) => s.meters);
  const [menu, setMenu] = useState<MenuState | null>(null);

  if (!track)
    return (
      <div className={styles.panel}>
        <div className={styles.title} data-surface="inspector-header">
          Select a track
        </div>
      </div>
    );
  const strip = stored ?? defaultStrip(track.kind);
  const buses = tracks.filter((t) => t.kind === "bus");
  const inputs = tracks.filter((t) => t.output === track.id);

  // Where the fader goes: the Stereo Out or a bus track (a group). Buses feed the Stereo Out.
  const openOutputMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const route = (output: string) =>
      store.fire("track.setOutput", { trackId: track.id, output });
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: [
        {
          label: "Stereo Out",
          checked: !track.output,
          onSelect: () => route("Stereo Out"),
        },
        ...buses.map((b) => ({
          label: b.name,
          checked: track.output === b.id,
          onSelect: () => route(b.id),
        })),
        separator,
        {
          label: "New Bus",
          onSelect: () => store.fire("track.group", { trackIds: [track.id] }),
        },
      ],
    });
  };

  // What a send feeds: A, B or (from a track) a bus track; sends 2 and 3 can be removed.
  const openSendMenu = (index: number) => (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const current = strip.sends[index]?.bus;
    const point = (bus: string) =>
      store.fire("strip.setSend", { trackId: track.id, send: index, bus });
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: [
        {
          label: "A · Reverb",
          checked: current === "bus-a",
          onSelect: () => point("A"),
        },
        {
          label: "B · Delay",
          checked: current === "bus-b",
          onSelect: () => point("B"),
        },
        ...(track.kind === "bus"
          ? []
          : buses.map((b) => ({
              label: b.name,
              checked: current === b.id,
              onSelect: () => point(b.id),
            }))),
        separator,
        {
          label: index >= 2 ? "Remove Send" : "Reset to Default, Off",
          onSelect: () => point("none"),
        },
      ],
    });
  };
  const openAddSendMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: buses.map((b) => ({
        label: b.name,
        onSelect: () =>
          store.fire("strip.setSend", {
            trackId: track.id,
            send: strip.sends.length,
            bus: b.id,
            levelDb: -6,
          }),
      })),
    });
  };
  const db = faderToDb(track.volume);
  const volume = (value: number) =>
    isBus
      ? store.fire("master.setVolume", { volume: value })
      : dispatch(
          commands.track.setVolume({ trackId: track.id, volume: value }),
        );

  const onFaderDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setFromPointer(e);
  };
  const onFaderMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.currentTarget.hasPointerCapture(e.pointerId)) setFromPointer(e);
  };
  const setFromPointer = (e: ReactPointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - r.top - size.faderCapH / 2;
    const position = Math.min(1, Math.max(0, 1 - y / faderTravel));
    volume(position);
  };

  const openInstrumentMenu = (e: MouseEvent<HTMLElement>) => {
    if (track.kind !== "midi") return;
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: store.catalog.instruments.map((name) => ({
        label: name,
        checked: strip.instrument === name,
        onSelect: () =>
          dispatch(
            commands.strip.setInstrument({
              trackId: track.id,
              instrument: name,
            }),
          ),
      })),
    });
  };

  const openInsertMenu =
    (slotIndex: number, slot: InsertSlot) => (e: MouseEvent<HTMLElement>) => {
      const r = e.currentTarget.getBoundingClientRect();
      setMenu({
        x: r.left,
        y: r.bottom + 4,
        anchor: e.currentTarget,
        items: [
          ...store.catalog.effects.map((name) => ({
            label: name,
            checked: slot.state !== "empty" && slot.name === name,
            onSelect: () =>
              dispatch(
                commands.strip.setInsert({
                  trackId: track.id,
                  slotIndex,
                  name,
                }),
              ),
          })),
          { separator: true as const },
          {
            label: slot.state === "bypassed" ? "Enable" : "Bypass",
            disabled: slot.state === "empty",
            onSelect: () =>
              dispatch(
                commands.strip.setInsertState({
                  trackId: track.id,
                  slotIndex,
                  state: slot.state === "bypassed" ? "active" : "bypassed",
                }),
              ),
          },
          {
            label: "Parameters",
            disabled: slot.state === "empty",
            onSelect: () =>
              store.fire("ui.openPluginWindow", {
                trackId: track.id,
                slot: slotIndex,
              }),
          },
          {
            label: "Open plugin window",
            disabled: slot.state === "empty",
            onSelect: () =>
              store.fire("ui.openPluginWindow", {
                trackId: track.id,
                slot: slotIndex,
                native: true,
              }),
          },
          {
            label: "Move up",
            disabled: slotIndex === 0 || slot.state === "empty",
            onSelect: () =>
              store.fire("strip.moveInsert", {
                trackId: track.id,
                from: slotIndex,
                to: slotIndex - 1,
              }),
          },
          {
            label: "Move down",
            disabled:
              slotIndex >=
                strip.inserts.findLastIndex((i) => i.state !== "empty") ||
              slot.state === "empty",
            onSelect: () =>
              store.fire("strip.moveInsert", {
                trackId: track.id,
                from: slotIndex,
                to: slotIndex + 1,
              }),
          },
          {
            label: "Remove",
            disabled: slot.state === "empty",
            onSelect: () =>
              dispatch(
                commands.strip.setInsertState({
                  trackId: track.id,
                  slotIndex,
                  state: "empty",
                }),
              ),
          },
        ],
      });
    };

  return (
    <div className={styles.panel}>
      <div className={styles.title} data-surface="inspector-header">
        <span
          className={`${styles.swatch} m-swatch`}
          style={{ background: track.color }}
        />
        <span className={styles.name}>{track.name}</span>
        <span className={styles.meta}>
          {isBus
            ? selected === "master"
              ? "MASTER"
              : "AUX BUS"
            : track.kind === "bus"
              ? `BUS · ${inputs.length} in`
              : track.kind === "midi"
                ? `MIDI · Ch ${store.getState().tracks.findIndex((t) => t.id === track.id) + 1}`
                : "AUDIO · In 1"}
        </span>
      </div>

      {!isBus && (
        <>
          <div className={styles.rows}>
            {track.kind !== "bus" && (
              <Row
                label="Instrument"
                value={strip.instrument}
                onClick={track.kind === "midi" ? openInstrumentMenu : undefined}
              />
            )}
            {track.kind === "midi" && (
              <button
                className="m-button"
                onClick={() =>
                  store.fire("ui.openPluginWindow", { trackId: track.id })
                }
              >
                Instrument parameters
              </button>
            )}
            {track.kind === "bus" ? (
              <Row
                label="Inputs"
                value={inputs.map((t) => t.name).join(", ") || "—"}
              />
            ) : (
              <Row label="Input" value={strip.input} />
            )}
            <Row
              label="Output"
              value={outputName(tracks, track)}
              onClick={track.kind === "bus" ? undefined : openOutputMenu}
            />
          </div>

          <div className={styles.section}>
            <div className={styles.sectionHead}>
              <CapsLabel>Channel EQ</CapsLabel>
              <span className={styles.sectionMeta}>
                {strip.inserts.some(
                  (i) => i.name === "Channel EQ" && i.state === "active",
                )
                  ? "active"
                  : "not inserted"}
              </span>
            </div>
            <EqDisplay
              active={strip.inserts.some(
                (i) => i.name === "Channel EQ" && i.state === "active",
              )}
            />
          </div>
        </>
      )}

      <div className={`${styles.section} ${styles.inserts}`}>
        <div className={styles.sectionHead}>
          <CapsLabel>Inserts</CapsLabel>
          <span className={styles.sectionMeta}>
            {strip.inserts.filter((i) => i.state !== "empty").length} / 8
          </span>
        </div>
        {strip.inserts
          .slice(
            0,
            Math.min(
              8,
              Math.max(
                1,
                strip.inserts.findLastIndex((i) => i.state !== "empty") + 2,
              ),
            ),
          )
          .map((ins, i) => (
            <Insert
              key={i}
              slot={ins}
              onClick={
                ins.state === "empty"
                  ? openInsertMenu(i, ins)
                  : () =>
                      store.fire("ui.openPluginWindow", {
                        trackId: track.id,
                        slot: i,
                      })
              }
              onContextMenu={openInsertMenu(i, ins)}
              onToggle={() =>
                ins.state !== "empty" &&
                dispatch(
                  commands.strip.setInsertState({
                    trackId: track.id,
                    slotIndex: i,
                    state: ins.state === "bypassed" ? "active" : "bypassed",
                  }),
                )
              }
            />
          ))}
      </div>

      {!isBus && (
        <div className={styles.section}>
          <div className={styles.sectionHead}>
            <CapsLabel>Sends</CapsLabel>
            {track.kind !== "bus" &&
              buses.length > 0 &&
              strip.sends.length < MAX_SENDS && (
                <button
                  className={`m-button ${styles.addSend}`}
                  onClick={openAddSendMenu}
                  title="Send to a bus"
                >
                  + Send
                </button>
              )}
          </div>
          <div className={styles.sends}>
            {strip.sends.map((s, i) => (
              <SendCard
                key={i}
                send={s}
                onPick={openSendMenu(i)}
                onChange={(levelDb) =>
                  dispatch(
                    commands.strip.setSendLevel({
                      trackId: track.id,
                      sendIndex: i,
                      levelDb,
                    }),
                  )
                }
              />
            ))}
          </div>
        </div>
      )}

      {(!isBus || selected === "master") && (
        <div className={styles.fader}>
          <div className={styles.faderLeft}>
            {!isBus && (
              <>
                <CapsLabel>Pan</CapsLabel>
                <Knob
                  size="lg"
                  angle={track.pan * PAN_DEGREES}
                  value={track.pan}
                  min={-100}
                  max={100}
                  defaultValue={0}
                  onChange={(pan) =>
                    dispatch(
                      commands.track.setPan({
                        trackId: track.id,
                        pan: Math.round(pan),
                      }),
                    )
                  }
                />
                <div className={styles.panValue}>{formatPan(track.pan)}</div>
              </>
            )}
            <CapsLabel style={{ marginTop: "auto" }}>Vol</CapsLabel>
            <div
              className={styles.volValue}
              onDoubleClick={() => volume(dbToFader(0))}
              title="Double-click for 0 dB"
            >
              {formatDb(db)}
            </div>
          </div>
          <div className={styles.faderRight}>
            <div
              className={`${styles.faderRail} m-groove`}
              onPointerDown={onFaderDown}
              onPointerMove={onFaderMove}
            >
              <div
                className={styles.faderCap}
                style={{ top: faderCapTop(track.volume) }}
              >
                <div className={styles.faderLine} />
              </div>
            </div>
            <div className={`${styles.channelMeter} m-well-meter`}>
              <LedStrip
                orientation="vertical"
                segments={CHANNEL_SEGMENTS}
                level={isBus ? meters.masterL : meters.channelL}
                hot={2}
              />
              <LedStrip
                orientation="vertical"
                segments={CHANNEL_SEGMENTS}
                level={isBus ? meters.masterR : meters.channelR}
                hot={2}
              />
            </div>
            <div className={styles.scale}>
              <span>+6</span>
              <span>0</span>
              <span>−6</span>
              <span>−12</span>
              <span>−24</span>
              <span>−∞</span>
            </div>
          </div>
        </div>
      )}
      {!isBus && (
        <div className={styles.channelButtons}>
          <button
            className="m-button"
            data-tone="mute"
            aria-pressed={track.mute}
            onClick={() =>
              store.fire("track.setMute", {
                trackId: track.id,
                muted: !track.mute,
              })
            }
          >
            M
          </button>
          <button
            className="m-button"
            data-tone="solo"
            aria-pressed={track.solo}
            onClick={() =>
              store.fire("track.setSolo", {
                trackId: track.id,
                solo: !track.solo,
              })
            }
          >
            S
          </button>
          {track.kind !== "bus" && (
            <button
              className="m-button"
              data-tone="arm"
              aria-label="Arm track"
              aria-pressed={track.armed}
              onClick={() =>
                store.fire("track.setArmed", {
                  trackId: track.id,
                  armed: !track.armed,
                })
              }
            >
              <RecordSmallIcon />
            </button>
          )}
          {track.kind === "audio" && (
            <button
              className="m-button"
              aria-label="Input monitoring"
              title={`Input monitoring: ${track.monitor ?? "off"}`}
              aria-pressed={(track.monitor ?? "off") !== "off"}
              onClick={() =>
                store.fire("track.setMonitor", {
                  trackId: track.id,
                  monitor: { off: "auto", auto: "on", on: "off" }[
                    track.monitor ?? "off"
                  ],
                })
              }
            >
              {track.monitor === "auto" ? "A" : "I"}
            </button>
          )}
          <span>{outputName(tracks, track)}</span>
        </div>
      )}
      {!isBus && <Region />}
      {menu && (
        <PopupMenu
          items={menu.items}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          anchor={menu.anchor}
        />
      )}
    </div>
  );
}

function Row({
  label,
  value,
  onClick,
}: {
  label: string;
  value: string;
  onClick?: ((e: MouseEvent<HTMLElement>) => void) | undefined;
}) {
  return (
    <div
      className={`${styles.row} ${onClick ? styles.rowClickable : ""}`}
      onClick={onClick}
      title={onClick ? "Click to change" : undefined}
    >
      <span className={styles.rowLabel}>{label}</span>
      <span className={styles.rowValue}>{value}</span>
    </div>
  );
}

function EqDisplay({ active }: { active: boolean }) {
  // Actual parameters are shown in the plugin window.
  const path = "M0 40 L212 40";
  return (
    <div className={`${styles.eq} m-well-deep`}>
      <div className={styles.eqGrid} />
      <svg
        width="212"
        height="74"
        viewBox="0 0 212 74"
        className={styles.eqSvg}
      >
        <path
          d={path}
          fill="none"
          stroke={
            active ? "var(--color-eq-curve)" : "var(--color-well-ink-faint)"
          }
          strokeWidth="1.5"
        />
      </svg>
    </div>
  );
}

function Insert({
  slot,
  onClick,
  onToggle,
  onContextMenu,
}: {
  slot: InsertSlot;
  onClick: (e: MouseEvent<HTMLElement>) => void;
  onToggle: () => void;
  onContextMenu: (e: MouseEvent<HTMLElement>) => void;
}) {
  const cls =
    slot.state === "active"
      ? styles.insertActive
      : slot.state === "bypassed"
        ? styles.insertBypassed
        : styles.insertEmpty;
  return (
    <div
      className={`${styles.insert} ${cls}`}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(e);
      }}
      title={
        slot.state === "empty"
          ? "Click to add an effect"
          : "Click to change · LED toggles bypass"
      }
    >
      <span
        className={styles.insertLed}
        onClick={(e) => {
          e.stopPropagation();
          onToggle();
        }}
      />
      {slot.state === "empty" ? "Add effect…" : slot.name}
      <span className={styles.insertMeta}>
        {slot.state === "bypassed" ? "bypassed" : slot.meta}
      </span>
    </div>
  );
}

function SendCard({
  send,
  onChange,
  onPick,
}: {
  send: Send;
  onChange: (levelDb: number) => void;
  onPick: (e: MouseEvent<HTMLElement>) => void;
}) {
  const value =
    send.levelDb === -Infinity
      ? SEND_MIN_DB
      : Math.max(SEND_MIN_DB, send.levelDb);
  return (
    <div className={styles.send}>
      <Knob
        size="md"
        angle={sendAngle(send.levelDb)}
        value={value}
        min={SEND_MIN_DB}
        max={0}
        defaultValue={SEND_MIN_DB}
        onChange={(v) =>
          onChange(v <= SEND_MIN_DB + 0.5 ? -Infinity : Math.round(v * 2) / 2)
        }
        title="Drag to set the send level"
      />
      <div className={styles.sendText}>
        <button
          className={styles.sendName}
          onClick={onPick}
          title="Choose where this send goes"
        >
          {send.name}
        </button>
        <span className={styles.sendValue}>{formatDb(send.levelDb)} dB</span>
      </div>
    </div>
  );
}

function Region() {
  const store = useStore();
  const clip = useSession((s) =>
    s.clips.find((c) => c.id === s.view.selectedClipId),
  );
  if (!clip) return null;
  const audio = clip.data.kind === "audio" ? clip.data : null;
  return (
    <div
      className={styles.section}
      key={`${clip.id}-${clip.name}-${clip.startBar}-${clip.lengthBars}-${JSON.stringify(audio)}`}
    >
      <div className={styles.sectionHead}>
        <CapsLabel>Region</CapsLabel>
        <span className={styles.sectionMeta}>
          {clip.data.kind === "midi"
            ? `${clip.data.notes.length} notes`
            : "Audio"}
        </span>
      </div>
      <input
        className="region-name m-well-deep"
        aria-label="Region name"
        defaultValue={clip.name}
        onBlur={(e) => {
          if (e.target.value.trim() && e.target.value !== clip.name)
            store.fire("clip.rename", {
              clipId: clip.id,
              name: e.target.value,
            });
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
      <div className="region-values">
        <label>
          Bar
          <input
            aria-label="Region start bar"
            type="number"
            min="1"
            step=".0625"
            defaultValue={clip.startBar + 1}
            onBlur={(e) => {
              // Tabbing through must not add an undo step that changes nothing.
              const startBar = e.currentTarget.valueAsNumber - 1;
              if (
                Number.isFinite(startBar) &&
                startBar >= 0 &&
                startBar !== clip.startBar
              )
                store.fire("clip.move", { clipId: clip.id, startBar });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
          />
        </label>
        <label>
          Length
          <input
            aria-label="Region length"
            type="number"
            min=".0625"
            step=".0625"
            defaultValue={clip.lengthBars}
            onBlur={(e) => {
              const lengthBars = e.currentTarget.valueAsNumber;
              if (
                Number.isFinite(lengthBars) &&
                lengthBars > 0 &&
                lengthBars !== clip.lengthBars
              )
                store.fire("clip.resize", { clipId: clip.id, lengthBars });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
          />
        </label>
      </div>
      {audio && <AudioRegion clipId={clip.id} data={audio} />}
    </div>
  );
}

/** Clip gain and fades of an audio region. Fades show in milliseconds; the host keeps seconds. */
function AudioRegion({
  clipId,
  data,
}: {
  clipId: string;
  data: AudioClipData;
}) {
  const dispatch = useDispatch();
  const env = clipEnvelope(data);
  const commit = (value: number, apply: (v: number) => void) => {
    if (Number.isFinite(value)) apply(value);
  };
  const fades = (fadeInSeconds: number, fadeOutSeconds: number) =>
    dispatch(commands.clip.setFades({ clipId, fadeInSeconds, fadeOutSeconds }));
  return (
    <>
      <div className="region-values">
        <label title="Clip gain, before the track's inserts">
          Gain
          <input
            aria-label="Region gain in dB"
            type="number"
            min={CLIP_GAIN_MIN_DB}
            max={CLIP_GAIN_MAX_DB}
            step=".5"
            defaultValue={Math.round(env.gainDb * 10) / 10}
            onBlur={(e) =>
              commit(e.currentTarget.valueAsNumber, (db) => {
                const gainDb = Math.max(
                  CLIP_GAIN_MIN_DB,
                  Math.min(CLIP_GAIN_MAX_DB, db),
                );
                if (gainDb !== env.gainDb)
                  dispatch(commands.clip.setGain({ clipId, gainDb }));
              })
            }
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
          />
          <span>dB</span>
        </label>
        <label title="Fade shape">
          <select
            aria-label="Fade curve"
            value={env.curve}
            onChange={(e) =>
              dispatch(
                commands.clip.setFades({
                  clipId,
                  curve: e.currentTarget.value as FadeCurve,
                }),
              )
            }
          >
            {FADE_CURVES.map((curve) => (
              <option key={curve} value={curve}>
                {FADE_CURVE_LABELS[curve]}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="region-values">
        <label
          className="region-wide"
          title="Fade in and fade out, in milliseconds"
        >
          Fades
          {(["in", "out"] as const).map((edge) => (
            <input
              key={edge}
              aria-label={`Fade ${edge} in milliseconds`}
              type="number"
              min="0"
              step="10"
              defaultValue={Math.round(
                (edge === "in" ? env.fadeIn : env.fadeOut) * 1000,
              )}
              onBlur={(e) =>
                commit(e.currentTarget.valueAsNumber, (ms) => {
                  const seconds = Math.max(0, ms) / 1000;
                  if (edge === "in" && seconds !== env.fadeIn)
                    fades(seconds, env.fadeOut);
                  if (edge === "out" && seconds !== env.fadeOut)
                    fades(env.fadeIn, seconds);
                })
              }
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
              }}
            />
          ))}
          <span>ms</span>
        </label>
      </div>
    </>
  );
}
