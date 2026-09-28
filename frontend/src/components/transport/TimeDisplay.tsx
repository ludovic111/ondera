import {
  useRef,
  useState,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  beatsToBarBeat,
  commands,
  formatSmpte,
  secondsToSmpte,
  tempoMap,
  tempoSourceBar,
} from "@ryolune/core";
import { useDispatch, useSession } from "../../state/session";
import { InlineEdit } from "../primitives/InlineEdit";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import styles from "./TimeDisplay.module.css";

const pad = (n: number, w: number) => String(n).padStart(w, "0");
/** Pixels of vertical drag per BPM. */
const TEMPO_DRAG_PX = 3;
const SIGNATURES: [number, number][] = [
  [4, 4],
  [3, 4],
  [6, 8],
  [5, 4],
  [7, 8],
  [2, 4],
  [12, 8],
];
const KEYS = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/** Sunk LCD-style readout. Tempo drags and double-clicks to edit; signature and key open pickers. */
export function TimeDisplay() {
  const dispatch = useDispatch();
  const positionBeats = useSession((s) => s.transport.positionBeats);
  const transport = useSession((s) => s.transport);
  const tempoChanges = useSession((s) => s.tempoChanges);
  const sig = transport.timeSignature;
  const song = { transport, tempoChanges };
  // The tempo playing at the playhead; with tempo changes, dragging or typing edits the one
  // in force there (bar 0 is the starting tempo).
  const map = tempoMap(song);
  const tempo = map.bpm(positionBeats);
  const sourceBar = tempoSourceBar(song, positionBeats);
  const setTempo = (bpm: number) =>
    dispatch(
      sourceBar === 0
        ? commands.transport.setTempo({ bpm })
        : commands.tempo.set({ bar: sourceBar, bpm }),
    );
  const key = useSession((s) => s.transport.key);
  const [editingTempo, setEditingTempo] = useState(false);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const tempoDrag = useRef<{ y: number; tempo: number } | null>(null);

  const pos = beatsToBarBeat(positionBeats, sig);
  const smpte = formatSmpte(secondsToSmpte(map.seconds(positionBeats)));

  const onTempoDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    // Drag from the tempo being edited, not from where a ramp has glided to.
    const own =
      sourceBar === 0
        ? transport.tempo
        : (tempoChanges.find((p) => p.bar === sourceBar)?.bpm ?? tempo);
    tempoDrag.current = { y: e.clientY, tempo: own };
  };
  const onTempoMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!tempoDrag.current || !e.currentTarget.hasPointerCapture(e.pointerId))
      return;
    const delta = (tempoDrag.current.y - e.clientY) / TEMPO_DRAG_PX;
    const next =
      Math.round(
        Math.min(
          400,
          Math.max(
            20,
            tempoDrag.current.tempo + (e.shiftKey ? delta / 10 : delta),
          ),
        ) * 10,
      ) / 10;
    if (next !== tempo) setTempo(next);
  };
  const onTempoUp = () => {
    tempoDrag.current = null;
  };

  const openSigMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: SIGNATURES.map(([n, d]) => ({
        label: `${n}/${d}`,
        checked: sig.numerator === n && sig.denominator === d,
        onSelect: () =>
          dispatch(
            commands.transport.setTimeSignature({
              numerator: n,
              denominator: d,
            }),
          ),
      })),
    });
  };
  const openKeyMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
      items: ["maj", "min"].flatMap((mode) =>
        KEYS.map((k) => ({
          label: `${k} ${mode}`,
          checked: key === `${k} ${mode}`,
          onSelect: () =>
            dispatch(commands.transport.setKey({ key: `${k} ${mode}` })),
        })),
      ),
    });
  };

  return (
    <div className={`${styles.display} m-well-deep`}>
      <div className={styles.cell}>
        <div className={styles.label}>Position</div>
        <div className={styles.position}>
          <span>{pad(pos.bar, 3)}</span>
          <span className={styles.dot}>·</span>
          <span>{pos.beat}</span>
          <span className={styles.dot}>·</span>
          <span>{pos.division}</span>
          <span className={styles.dot}>·</span>
          <span className={styles.ticks}>{pad(pos.tick, 3)}</span>
        </div>
      </div>
      <div className={`${styles.cell} ${styles.smpte}`}>
        <div className={styles.label}>SMPTE</div>
        <div className={`${styles.value} ${styles.dim}`}>{smpte}</div>
      </div>
      <div
        className={`${styles.cell} ${styles.editable}`}
        title={
          tempoChanges.length > 0
            ? `Tempo at the playhead, set at bar ${sourceBar + 1} · drag to change · double-click to type`
            : "Drag to change tempo · double-click to type"
        }
        onPointerDown={editingTempo ? undefined : onTempoDown}
        onPointerMove={onTempoMove}
        onPointerUp={onTempoUp}
        onDoubleClick={() => setEditingTempo(true)}
      >
        <div className={styles.label}>Tempo</div>
        {editingTempo ? (
          <InlineEdit
            mono
            className={styles.tempoInput}
            value={tempo.toFixed(2)}
            onCommit={(text) => {
              const bpm = Number.parseFloat(text);
              if (Number.isFinite(bpm))
                setTempo(Math.min(400, Math.max(20, bpm)));
              setEditingTempo(false);
            }}
            onCancel={() => setEditingTempo(false)}
          />
        ) : (
          <div className={styles.value}>{tempo.toFixed(2)}</div>
        )}
      </div>
      <div
        className={`${styles.cell} ${styles.editable}`}
        onClick={openSigMenu}
        title="Time signature"
      >
        <div className={styles.label}>Sig</div>
        <div className={styles.value}>
          {sig.numerator}/{sig.denominator}
        </div>
      </div>
      <div
        className={`${styles.cell} ${styles.last} ${styles.editable}`}
        onClick={openKeyMenu}
        title="Key"
      >
        <div className={styles.label}>Key</div>
        <div className={styles.value}>{key}</div>
      </div>
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
