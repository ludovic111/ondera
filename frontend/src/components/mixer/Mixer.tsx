import {
  useSyncExternalStore,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  commands,
  dbToFader,
  faderToDb,
  formatDb,
  type Track,
} from "@ondera/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { Knob } from "../primitives/Knob";
import { LedStrip } from "../primitives/LedStrip";
import { Button } from "../primitives/Button";
import { RecordSmallIcon } from "../primitives/Icons";
import { size } from "../../theme/tokens";
import styles from "./Mixer.module.css";

const PAN_DEGREES = 1.35;
const SEGMENTS = 20;
/** Shorter than the inspector's fader so a strip fits the editor pane's height. */
const FADER_H = 116;
const travel = FADER_H - size.faderCapH;
const formatPan = (pan: number) =>
  pan === 0 ? "C" : `${pan < 0 ? "L" : "R"} ${Math.abs(Math.round(pan))}`;

function Fader({
  position,
  onChange,
}: {
  position: number;
  onChange: (position: number) => void;
}) {
  const set = (e: ReactPointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - r.top - size.faderCapH / 2;
    onChange(Math.min(1, Math.max(0, 1 - y / travel)));
  };
  return (
    <div
      className={`${styles.rail} m-groove`}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        set(e);
      }}
      onPointerMove={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) set(e);
      }}
      onDoubleClick={() => onChange(dbToFader(0))}
      title="Drag to set the level · double-click for 0 dB"
    >
      <div className={styles.cap} style={{ top: (1 - position) * travel }}>
        <div className={styles.capLine} />
      </div>
    </div>
  );
}

function Strip({ track, index }: { track: Track; index: number }) {
  const store = useStore();
  const dispatch = useDispatch();
  const selected = useSession((s) => s.view.selectedTrackId === track.id);
  // Meters ride on the transport telemetry, which notifies session subscribers.
  const peak = useSession(() => store.trackPeaks[index] ?? 0);
  const inserts = useSession(
    (s) =>
      s.strips[track.id]?.inserts.filter((i) => i.state !== "empty").length ??
      0,
  );
  return (
    <div
      className={styles.strip}
      data-selected={selected}
      onPointerDown={() =>
        selected || dispatch(commands.track.select({ trackId: track.id }))
      }
    >
      <div className={styles.kind}>
        {track.kind === "midi" ? "MIDI" : "AUD"} · {inserts} fx
      </div>
      <Knob
        size="md"
        title={`Pan ${formatPan(track.pan)}`}
        angle={track.pan * PAN_DEGREES}
        value={track.pan}
        min={-100}
        max={100}
        defaultValue={0}
        onChange={(pan) =>
          dispatch(
            commands.track.setPan({ trackId: track.id, pan: Math.round(pan) }),
          )
        }
      />
      <div className={styles.faderRow}>
        <Fader
          position={track.volume}
          onChange={(volume) =>
            dispatch(commands.track.setVolume({ trackId: track.id, volume }))
          }
        />
        <div className={`${styles.meter} m-well-meter`}>
          <LedStrip
            orientation="vertical"
            segments={SEGMENTS}
            level={track.mute ? 0 : peak}
            hot={2}
          />
        </div>
      </div>
      <div className={styles.value}>{formatDb(faderToDb(track.volume))} dB</div>
      <div className={styles.buttons}>
        <Button
          size="sm"
          title="Mute"
          pressed={track.mute}
          onClick={() =>
            dispatch(
              commands.track.setMute({ trackId: track.id, muted: !track.mute }),
            )
          }
        >
          M
        </Button>
        <Button
          size="sm"
          title="Solo"
          pressed={track.solo}
          onClick={() =>
            dispatch(
              commands.track.setSolo({ trackId: track.id, solo: !track.solo }),
            )
          }
        >
          S
        </Button>
        <Button
          size="sm"
          title="Record arm"
          lit={track.armed}
          onClick={() =>
            dispatch(
              commands.track.setArmed({
                trackId: track.id,
                armed: !track.armed,
              }),
            )
          }
        >
          <RecordSmallIcon />
        </Button>
      </div>
      <div className={styles.name} title={track.name}>
        <span className={styles.swatch} style={{ background: track.color }} />
        {track.name}
      </div>
    </div>
  );
}

function MasterStrip() {
  const store = useStore();
  const volume = useSyncExternalStore(
    store.subscribeMeta,
    () => store.document?.masterVolume ?? dbToFader(0),
  );
  const meters = useSession((s) => s.meters);
  return (
    <div className={`${styles.strip} ${styles.master}`}>
      <div className={styles.kind}>Stereo out</div>
      <button
        className="m-button"
        title="Open the master strip in the inspector"
        onClick={() =>
          store.fire("ui.showPanel", { panel: "master", visible: true })
        }
      >
        Inserts…
      </button>
      <div className={styles.faderRow}>
        <Fader
          position={volume}
          onChange={(v) => store.fire("master.setVolume", { volume: v })}
        />
        <div className={`${styles.meter} m-well-meter`}>
          <LedStrip
            orientation="vertical"
            segments={SEGMENTS}
            level={meters.masterL}
            hot={2}
          />
          <LedStrip
            orientation="vertical"
            segments={SEGMENTS}
            level={meters.masterR}
            hot={2}
          />
        </div>
      </div>
      <div className={styles.value}>{formatDb(faderToDb(volume))} dB</div>
      <div className={styles.buttons} />
      <div className={styles.name}>Master</div>
    </div>
  );
}

/** Every channel side by side, in place of the region editor. Same commands as the inspector. */
export function Mixer() {
  const store = useStore();
  const tracks = useSession((s) => s.tracks);
  return (
    <div className={styles.pane} data-surface="mixer">
      <div className={styles.header}>
        <strong>Mixer</strong>
        <span className={styles.dim}>
          {tracks.length} {tracks.length === 1 ? "channel" : "channels"} · X
          toggles
        </span>
        <button
          className="plain-control"
          onClick={() =>
            store.fire("ui.showPanel", { panel: "mixer", visible: false })
          }
        >
          Show editor
        </button>
      </div>
      <div className={styles.strips}>
        {tracks.map((t, i) => (
          <Strip key={t.id} track={t} index={i} />
        ))}
        {tracks.length === 0 && (
          <p className={styles.empty}>Add a track to see its channel here.</p>
        )}
        <MasterStrip />
      </div>
    </div>
  );
}
