import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { commands, snapBeats } from "@ondera/core";
import { useDispatch, useSession, useStore } from "../../state/session";
import { useCanvasSurface } from "../../canvas/surface";
import { beatAtX, editorClip, rollGeometry } from "../../canvas/pianoRoll";
import {
  LANE_CHOICES,
  drawControllerLane,
  hitPoint,
  laneParams,
  lanePoints,
  lanesInClip,
  laneTitle,
  parseCcNumber,
  sameLane,
  strokePoints,
  valueAtY,
  type Lane,
  type LaneOverlay,
  type Sample,
} from "../../canvas/controllerLane";
import { Button } from "../primitives/Button";
import { InlineEdit } from "../primitives/InlineEdit";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import type { MenuEntry } from "../../state/menus";
import styles from "./EditorPane.module.css";

const DRAG_THRESHOLD_PX = 3;

type Drag =
  | {
      kind: "point";
      id: string;
      startX: number;
      startY: number;
      moved: boolean;
      time: number;
      value: number;
      origTime: number;
      origValue: number;
    }
  | {
      kind: "stroke";
      startX: number;
      startY: number;
      moved: boolean;
      fine: boolean;
      samples: Sample[];
    };

/** The short name on the lane's selector; the canvas spells it out. */
function shortTitle(lane: Lane): string {
  if (lane.kind === "bend") return "Bend";
  if (lane.kind === "pressure") return "Press.";
  return `CC${lane.number}`;
}

/**
 * Controller lane under the piano roll: one controller at a time, its values as
 * steps with a stem per point. Click adds a point, dragging a point moves it,
 * dragging elsewhere draws, Option-click or right-click deletes. Drags are
 * previewed here and committed as one command on release, so each is one undo step.
 */
export function ControllerLane() {
  const dispatch = useDispatch();
  const store = useStore();
  const clip = useSession((s) => editorClip(s));
  const [lane, setLane] = useState<Lane>(LANE_CHOICES[0]!);
  const [overlay, setOverlay] = useState<LaneOverlay>({});
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [typing, setTyping] = useState(false);
  const drag = useRef<Drag | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);

  // Opening a clip shows a lane it uses when the current one is empty there.
  const clipId = clip?.id;
  useEffect(() => {
    const clip = editorClip(store.getState());
    const present = lanesInClip(clip);
    setLane((current) =>
      present.length > 0 && !present.some((l) => sameLane(l, current))
        ? present[0]!
        : current,
    );
  }, [clipId, store]);

  const canvasRef = useCanvasSurface(
    useCallback(
      (ctx, w, h) =>
        drawControllerLane(ctx, w, h, store.getState(), lane, overlay),
      [store, lane, overlay],
    ),
  );

  const frame = (e: { clientX: number; clientY: number }) => {
    const r = gridRef.current!.getBoundingClientRect();
    const geo = rollGeometry(store.getState(), r.width);
    return {
      x: e.clientX - r.left,
      y: e.clientY - r.top,
      h: r.height,
      geo,
      length: (geo.clip?.lengthBars ?? 0) * geo.bpb,
    };
  };
  const grid = () => 4 / store.getState().transport.snapDivision;
  /** A time inside the clip: snapped unless `fine`, and never at or past its end. */
  const timeAt = (beat: number, length: number, fine: boolean) => {
    const snapped = fine
      ? beat
      : snapBeats(beat, store.getState().transport.snapDivision);
    const last = fine ? length - 1e-3 : length - grid();
    return Math.max(0, Math.min(snapped, Math.max(0, last)));
  };

  const remove = (id: string) => {
    if (clip)
      dispatch(
        commands.controller.remove({ clipId: clip.id, controllerId: id }),
      );
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || !clip || clip.data.kind !== "midi") return;
    const f = frame(e);
    if (f.length <= 0) return;
    const hit = hitPoint(
      lanePoints(clip, lane),
      lane,
      f.geo.pxPerBeat,
      f.h,
      f.x,
      f.y,
    );
    if (hit && e.altKey) {
      remove(hit.id);
      return;
    }
    drag.current = hit
      ? {
          kind: "point",
          id: hit.id,
          startX: f.x,
          startY: f.y,
          moved: false,
          time: hit.time,
          value: hit.value,
          origTime: hit.time,
          origValue: hit.value,
        }
      : {
          kind: "stroke",
          startX: f.x,
          startY: f.y,
          moved: false,
          fine: e.altKey,
          samples: [
            {
              beat: Math.max(0, beatAtX(f.geo, f.x)),
              value: valueAtY(lane, f.y, f.h),
            },
          ],
        };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!clip) return;
    const f = frame(e);
    const d = drag.current;
    if (!d) {
      const hit = hitPoint(
        lanePoints(clip, lane),
        lane,
        f.geo.pxPerBeat,
        f.h,
        f.x,
        f.y,
      );
      e.currentTarget.style.cursor = hit ? "grab" : "crosshair";
      if (hit?.id !== overlay.hover) setOverlay(hit ? { hover: hit.id } : {});
      return;
    }
    if (
      !d.moved &&
      Math.hypot(f.x - d.startX, f.y - d.startY) < DRAG_THRESHOLD_PX
    )
      return;
    d.moved = true;
    if (d.kind === "point") {
      d.time = timeAt(beatAtX(f.geo, f.x), f.length, e.altKey);
      d.value = valueAtY(lane, f.y, f.h);
      setOverlay({ drag: { id: d.id, time: d.time, value: d.value } });
      return;
    }
    d.samples.push({
      beat: Math.max(0, Math.min(f.length, beatAtX(f.geo, f.x))),
      value: valueAtY(lane, f.y, f.h),
    });
    const stroke = strokePoints(
      d.samples,
      d.fine ? grid() / 4 : grid(),
      f.length,
    );
    setOverlay(stroke ? { stroke: stroke.points } : {});
  };

  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    drag.current = null;
    setOverlay({});
    if (!d || !clip) return;
    if (d.kind === "point") {
      if (d.moved && (d.time !== d.origTime || d.value !== d.origValue))
        dispatch(
          commands.controller.update({
            clipId: clip.id,
            controllerId: d.id,
            time: d.time,
            value: d.value,
          }),
        );
      return;
    }
    const f = frame(e);
    if (!d.moved) {
      const first = d.samples[0]!;
      dispatch(
        commands.controller.add({
          clipId: clip.id,
          ...laneParams(lane),
          time: timeAt(first.beat, f.length, d.fine),
          value: first.value,
        }),
      );
      return;
    }
    const stroke = strokePoints(
      d.samples,
      d.fine ? grid() / 4 : grid(),
      f.length,
    );
    if (!stroke) return;
    dispatch(
      commands.controller.setPoints({
        clipId: clip.id,
        ...laneParams(lane),
        points: stroke.points.map((p) => ({ time: p.beat, value: p.value })),
        from: stroke.from,
        to: stroke.to,
      }),
    );
  };

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (!clip) return;
    const f = frame(e);
    const hit = hitPoint(
      lanePoints(clip, lane),
      lane,
      f.geo.pxPerBeat,
      f.h,
      f.x,
      f.y,
    );
    if (hit) remove(hit.id);
  };

  const openMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const used = lanesInClip(clip);
    const choices = [...LANE_CHOICES];
    for (const lane of used)
      if (!choices.some((l) => sameLane(l, lane))) choices.push(lane);
    const items: MenuEntry[] = choices.map((choice) => ({
      label: `${laneTitle(choice)}${used.some((l) => sameLane(l, choice)) ? " ·" : ""}`,
      checked: sameLane(choice, lane),
      onSelect: () => setLane(choice),
    }));
    items.push(
      { separator: true },
      { label: "Other CC…", onSelect: () => setTyping(true) },
    );
    setMenu({ x: r.left, y: r.bottom + 4, items });
  };

  return (
    <div className={styles.lane} data-surface="controller-lane">
      <div className={styles.laneHead}>
        {typing ? (
          <InlineEdit
            value="CC "
            mono
            className={styles.laneInput}
            onCommit={(text) => {
              const number = parseCcNumber(text);
              if (number !== null) setLane({ kind: "cc", number });
              setTyping(false);
            }}
            onCancel={() => setTyping(false)}
          />
        ) : (
          <Button
            size="auto"
            className={styles.laneButton}
            onClick={openMenu}
            title={`${laneTitle(lane)}: choose the controller this lane shows`}
          >
            {shortTitle(lane)}
          </Button>
        )}
      </div>
      <div
        ref={gridRef}
        className={styles.laneGrid}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => {
          if (!drag.current && overlay.hover) setOverlay({});
        }}
        onContextMenu={onContextMenu}
      >
        <canvas ref={canvasRef} className={styles.canvas} />
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
