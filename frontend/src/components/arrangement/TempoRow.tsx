import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  barsToBeats,
  commands,
  snapBars,
  snapStepBars,
  tempoAt as tempoAtBeat,
  tempoSourceBar,
  type Session,
} from "@ryolune/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { useCanvasSurface } from "../../canvas/surface";
import { laneGeometry, xToBar, barToX } from "../../canvas/timeline";
import {
  bpmAtY,
  bpmLabel,
  drawTempoLane,
  tempoAt,
  tempoPoints,
  tempoRange,
  yOfBpm,
  type TempoDrag,
} from "../../canvas/tempoLane";
import { CapsLabel } from "../primitives/CapsLabel";
import { InlineEdit } from "../primitives/InlineEdit";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import { actionItem, separator } from "../../state/menus";
import { size } from "../../theme/tokens";
import styles from "./Arrangement.module.css";

const DRAG_THRESHOLD_PX = 3;

type Gesture =
  | { kind: "add"; startX: number }
  | {
      kind: "point";
      from: number;
      startX: number;
      startY: number;
      /** Bars between the pointer and the point when it was grabbed. */
      grab: number;
      /** The lane's BPM range when the drag began; it stays put while dragging. */
      range: [number, number];
      moved: boolean;
      drag: TempoDrag;
    };

/** Whole BPM, or tenths with Shift. */
const roundBpm = (bpm: number, fine: boolean) =>
  fine ? Math.round(bpm * 10) / 10 : Math.round(bpm);

/**
 * The tempo track under the ruler (View > Show Tempo Track, `ui.showPanel panel=tempo`).
 * Click to add a change, drag a point to move it or change its tempo, right-click to make
 * it ramp. What is in progress is a local overlay; the command goes out on release, so each
 * gesture is one undo step.
 */
export function TempoRow() {
  const store = useStore();
  const dispatch = useDispatch();
  const transport = useSession((s) => s.transport);
  const tempoChanges = useSession((s) => s.tempoChanges);
  const [overlay, setOverlay] = useState<TempoDrag | undefined>();
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [editing, setEditing] = useState<number | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const canvasRef = useCanvasSurface(
    useCallback(
      (ctx, w, h) => drawTempoLane(ctx, w, h, store.getState(), overlay),
      [store, overlay],
    ),
  );
  // A dropped point stays where it was left until the host's document places it there.
  useEffect(() => {
    if (!gesture.current) setOverlay(undefined);
  }, [tempoChanges, transport.tempo]);

  const local = (e: { clientX: number; clientY: number; currentTarget: Element }) => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top, h: r.height };
  };
  const snapped = (state: Session, bar: number, free: boolean) =>
    free
      ? bar
      : snapBars(bar, state.transport.snapDivision, state.transport.timeSignature);

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const state = store.getState();
    const { x, y, h } = local(e);
    const hit = tempoAt(state, x, y, h);
    if (hit) {
      const point = tempoPoints(state).find((p) => p.bar === hit.bar)!;
      gesture.current = {
        kind: "point",
        from: hit.bar,
        startX: x,
        startY: y,
        grab: xToBar(x, laneGeometry(state)) - hit.bar,
        range: tempoRange(tempoPoints(state)),
        moved: false,
        drag: { from: hit.bar, bar: hit.bar, bpm: point.bpm },
      };
    } else {
      gesture.current = { kind: "add", startX: x };
    }
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const state = store.getState();
    const { x, y, h } = local(e);
    const g = gesture.current;
    if (!g) {
      e.currentTarget.style.cursor = tempoAt(state, x, y, h)
        ? "grab"
        : "copy";
      return;
    }
    if (g.kind !== "point") return;
    if (
      !g.moved &&
      Math.hypot(x - g.startX, y - g.startY) < DRAG_THRESHOLD_PX
    )
      return;
    g.moved = true;
    e.currentTarget.style.cursor = "grabbing";
    const bpm = roundBpm(bpmAtY(y, g.range, h), e.shiftKey);
    // The starting tempo stays at bar 1; a change keeps after it.
    const { snapDivision, timeSignature } = state.transport;
    const first = e.altKey ? 0.001 : snapStepBars(snapDivision, timeSignature);
    const bar =
      g.from === 0
        ? 0
        : Math.max(
            first,
            snapped(state, xToBar(x, laneGeometry(state)) - g.grab, e.altKey),
          );
    g.drag = { from: g.from, bar, bpm };
    setOverlay({ ...g.drag });
  };

  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const g = gesture.current;
    gesture.current = null;
    if (!g) return;
    const state = store.getState();
    if (g.kind === "add") {
      const { x, y, h } = local(e);
      if (Math.abs(x - g.startX) >= DRAG_THRESHOLD_PX) return;
      const bar = snapped(state, xToBar(x, laneGeometry(state)), e.altKey);
      if (bar <= 1e-6) return;
      const range = tempoRange(tempoPoints(state));
      dispatch(
        commands.tempo.set({ bar, bpm: roundBpm(bpmAtY(y, range, h), e.shiftKey) }),
      );
      return;
    }
    e.currentTarget.style.cursor = "grab";
    if (!g.moved) {
      setOverlay(undefined);
      return;
    }
    const { from, bar, bpm } = g.drag;
    const taken = state.tempoChanges.some(
      (p) => p.bar !== from && Math.abs(p.bar - bar) < 1e-6,
    );
    if (taken) {
      setOverlay(undefined);
      return;
    }
    dispatch(
      from === 0 || Math.abs(bar - from) < 1e-6
        ? commands.tempo.set({ bar: from, bpm })
        : commands.tempo.move({ bar: from, toBar: bar, bpm }),
    );
    // If the host refuses the edit, the point goes home.
    setTimeout(() => {
      if (!gesture.current) setOverlay(undefined);
    }, 1000);
  };

  const onPointerCancel = () => {
    gesture.current = null;
    setOverlay(undefined);
  };

  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    const { x, y, h } = local(e);
    const hit = tempoAt(store.getState(), x, y, h);
    if (hit) setEditing(hit.bar);
  };

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const state = store.getState();
    const { x, y, h } = local(e);
    const hit = tempoAt(state, x, y, h);
    const point = hit
      ? state.tempoChanges.find((p) => p.bar === hit.bar)
      : undefined;
    const clear = {
      label: "Clear Tempo Changes",
      disabled: state.tempoChanges.length === 0,
      onSelect: () => dispatch(commands.tempo.clear({})),
    };
    const hide = actionItem(store, "toggleTempoTrack", "Hide Tempo Track");
    if (point) {
      setMenu({
        x: e.clientX,
        y: e.clientY,
        items: [
          {
            label: "Ramp from the Previous Tempo",
            checked: point.ramp ?? false,
            onSelect: () =>
              dispatch(
                commands.tempo.set({
                  bar: point.bar,
                  bpm: point.bpm,
                  ramp: !point.ramp,
                }),
              ),
          },
          { label: "Set Tempo…", onSelect: () => setEditing(point.bar) },
          separator,
          {
            label: "Delete Tempo Change",
            onSelect: () => dispatch(commands.tempo.remove({ bar: point.bar })),
          },
          separator,
          clear,
          hide,
        ],
      });
      return;
    }
    const bar = snapped(state, xToBar(x, laneGeometry(state)), false);
    const bpm = Math.round(
      tempoAtBeat(state, barsToBeats(bar, state.transport.timeSignature)),
    );
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: "Add Tempo Change Here",
          disabled:
            bar <= 1e-6 ||
            state.tempoChanges.some((p) => Math.abs(p.bar - bar) < 1e-6),
          onSelect: () => dispatch(commands.tempo.set({ bar, bpm })),
        },
        {
          label: hit ? "Set Starting Tempo…" : "Set Tempo…",
          onSelect: () =>
            setEditing(
              hit
                ? 0
                : tempoSourceBar(
                    state,
                    barsToBeats(bar, state.transport.timeSignature),
                  ),
            ),
        },
        separator,
        clear,
        hide,
      ],
    });
  };

  const editingStyle = (() => {
    if (editing === null) return undefined;
    const state = store.getState();
    const points = tempoPoints(state);
    const point = points.find((p) => p.bar === editing);
    if (!point) return undefined;
    const range = tempoRange(points);
    return {
      left: Math.max(2, barToX(point.bar, laneGeometry(state)) + 4),
      top: Math.max(
        0,
        Math.min(
          size.tempoLane - size.inlineInputH,
          yOfBpm(point.bpm, range, size.tempoLane) - size.inlineInputH / 2,
        ),
      ),
      width: 64,
    };
  })();
  const editingValue =
    editing === null
      ? ""
      : bpmLabel(
          tempoPoints(store.getState()).find((p) => p.bar === editing)?.bpm ??
            transport.tempo,
        );

  const playing = tempoAtBeat(
    { transport, tempoChanges },
    transport.positionBeats,
  );

  return (
    <div className={styles.tempoRow}>
      <div className={styles.rulerCorner}>
        <CapsLabel>Tempo</CapsLabel>
        <span
          className={styles.tempoValue}
          title="Tempo at the playhead"
        >
          {bpmLabel(playing)} BPM
        </span>
      </div>
      <div
        className={styles.rulerCanvasWrap}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
        onLostPointerCapture={onPointerCancel}
        onDoubleClick={onDoubleClick}
        onContextMenu={onContextMenu}
        title="Click to add a tempo change · drag a point to move it or change its tempo (Shift for tenths, Option off the grid) · double-click to type · right-click to ramp"
      >
        <canvas ref={canvasRef} className={styles.canvas} />
        {editing !== null && editingStyle && (
          <InlineEdit
            mono
            className={styles.renameInput}
            style={editingStyle}
            value={editingValue}
            onCommit={(text) => {
              const bpm = Number.parseFloat(text);
              if (Number.isFinite(bpm))
                dispatch(
                  commands.tempo.set({
                    bar: editing,
                    bpm: Math.min(400, Math.max(20, bpm)),
                  }),
                );
              setEditing(null);
            }}
            onCancel={() => setEditing(null)}
          />
        )}
      </div>
      {menu && (
        <PopupMenu
          items={menu.items}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
