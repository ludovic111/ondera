import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { barsToBeats, commands, snapBars, type Marker } from "@ondera/core";
import { useSession, useStore } from "../../state/session";
import { barToX, laneGeometry, xToBar } from "../../canvas/timeline";
import { markerAt } from "../../canvas/markers";
import type { RulerOverlay } from "../../canvas/ruler";
import { size } from "../../theme/tokens";

const DRAG_THRESHOLD_PX = 4;

type Drag =
  | { kind: "pending"; startX: number; anchorBar: number }
  | { kind: "range"; anchorBar: number; startBar: number; endBar: number }
  | { kind: "edge"; edge: "start" | "end"; startBar: number; endBar: number }
  | {
      kind: "marker";
      marker: Marker;
      startX: number;
      /** Bars between the pointer and the marker when it was grabbed. */
      grab: number;
      bar: number;
      moved: boolean;
    };

/**
 * Ruler gestures: click locates, drag defines a cycle range, dragging a
 * cycle edge trims it. A marker flag locates on click and moves on drag
 * (on the grid unless Option is held). What is in progress is a local
 * overlay; the command goes out on release, so each gesture is one undo step.
 */
export function useRulerInteraction() {
  const store = useStore();
  const [overlay, setOverlay] = useState<RulerOverlay>({});
  const drag = useRef<Drag | null>(null);
  // A moved flag stays where it was dropped until the host's document places it there.
  const markers = useSession((s) => s.markers);
  useEffect(() => {
    if (!drag.current) setOverlay((o) => (o.marker ? {} : o));
  }, [markers]);

  const local = (e: {
    clientX: number;
    clientY: number;
    currentTarget: Element;
  }) => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  const onPointerDown = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      const state = store.getState();
      const geo = laneGeometry(state);
      const { x, y } = local(e);
      const t = state.transport;
      const marker = markerAt(state, x, y);
      if (marker) {
        drag.current = {
          kind: "marker",
          marker,
          startX: x,
          grab: xToBar(x, geo) - marker.bar,
          bar: marker.bar,
          moved: false,
        };
      } else if (t.cycle) {
        const x0 = barToX(t.cycleStartBar, geo);
        const x1 = barToX(t.cycleEndBar, geo);
        if (Math.abs(x - x0) <= size.cycleGrip)
          drag.current = {
            kind: "edge",
            edge: "start",
            startBar: t.cycleStartBar,
            endBar: t.cycleEndBar,
          };
        else if (Math.abs(x - x1) <= size.cycleGrip)
          drag.current = {
            kind: "edge",
            edge: "end",
            startBar: t.cycleStartBar,
            endBar: t.cycleEndBar,
          };
      }
      if (!drag.current) {
        drag.current = {
          kind: "pending",
          startX: x,
          anchorBar: Math.max(0, Math.round(xToBar(x, geo))),
        };
      }
      e.currentTarget.setPointerCapture(e.pointerId);
    },
    [store],
  );

  const onPointerMove = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      const state = store.getState();
      const geo = laneGeometry(state);
      const { x, y } = local(e);
      const d = drag.current;
      if (!d) {
        const t = state.transport;
        const nearEdge =
          t.cycle &&
          (Math.abs(x - barToX(t.cycleStartBar, geo)) <= size.cycleGrip ||
            Math.abs(x - barToX(t.cycleEndBar, geo)) <= size.cycleGrip);
        e.currentTarget.style.cursor = markerAt(state, x, y)
          ? "grab"
          : nearEdge
            ? "ew-resize"
            : "default";
        return;
      }
      if (d.kind === "marker") {
        if (!d.moved && Math.abs(x - d.startX) < DRAG_THRESHOLD_PX) return;
        d.moved = true;
        e.currentTarget.style.cursor = "grabbing";
        const raw = Math.max(0, xToBar(x, geo) - d.grab);
        d.bar = e.altKey
          ? raw
          : Math.max(
              0,
              snapBars(
                raw,
                state.transport.snapDivision,
                state.transport.timeSignature,
              ),
            );
        setOverlay({ marker: { id: d.marker.id, bar: d.bar } });
        return;
      }
      const bar = Math.max(0, Math.round(xToBar(x, geo)));
      let cur: Drag = d;
      if (d.kind === "pending") {
        if (Math.abs(x - d.startX) < DRAG_THRESHOLD_PX) return;
        cur = {
          kind: "range",
          anchorBar: d.anchorBar,
          startBar: Math.min(d.anchorBar, bar),
          endBar: Math.max(d.anchorBar, bar),
        };
        drag.current = cur;
      }
      if (cur.kind === "range") {
        cur.startBar = Math.min(cur.anchorBar, bar);
        cur.endBar = Math.max(cur.anchorBar, bar, cur.startBar + 1);
        setOverlay({ cycle: { startBar: cur.startBar, endBar: cur.endBar } });
      } else if (cur.kind === "edge") {
        if (cur.edge === "start") cur.startBar = Math.min(bar, cur.endBar - 1);
        else cur.endBar = Math.max(bar, cur.startBar + 1);
        setOverlay({ cycle: { startBar: cur.startBar, endBar: cur.endBar } });
      }
    },
    [store],
  );

  const onPointerUp = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      drag.current = null;
      if (!d) return;
      const state = store.getState();
      if (d.kind === "marker") {
        e.currentTarget.style.cursor = "grab";
        if (!d.moved) {
          setOverlay({});
          store.dispatch(commands.marker.goto({ markerId: d.marker.id }));
          return;
        }
        const taken = state.markers.some(
          (m) => m.id !== d.marker.id && Math.abs(m.bar - d.bar) < 1e-6,
        );
        if (d.bar === d.marker.bar || taken) {
          setOverlay({});
          return;
        }
        store.dispatch(
          commands.marker.move({ markerId: d.marker.id, bar: d.bar }),
        );
        // If the host refuses the move, the flag goes home.
        setTimeout(() => {
          if (!drag.current) setOverlay((o) => (o.marker ? {} : o));
        }, 1000);
        return;
      }
      setOverlay({});
      if (d.kind === "pending") {
        const geo = laneGeometry(state);
        const raw = Math.max(0, xToBar(local(e).x, geo));
        const bar = e.altKey
          ? raw
          : snapBars(
              raw,
              state.transport.snapDivision,
              state.transport.timeSignature,
            );
        store.dispatch(
          commands.transport.setPosition({
            beats: barsToBeats(bar, state.transport.timeSignature),
          }),
        );
        return;
      }
      store.dispatch(
        commands.transport.setCycleRange({
          startBar: d.startBar,
          endBar: d.endBar,
        }),
      );
      if (!state.transport.cycle)
        store.dispatch(commands.transport.setCycle({ enabled: true }));
    },
    [store],
  );

  return { overlay, onPointerDown, onPointerMove, onPointerUp };
}
