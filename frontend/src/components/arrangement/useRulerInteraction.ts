import {
  useCallback,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { barsToBeats, commands, snapBars } from "@ondera/core";
import { useStore } from "../../state/session";
import { barToX, laneGeometry, xToBar } from "../../canvas/timeline";
import type { RulerOverlay } from "../../canvas/ruler";
import { size } from "../../theme/tokens";

const DRAG_THRESHOLD_PX = 4;

type Drag =
  | { kind: "pending"; startX: number; anchorBar: number }
  | { kind: "range"; anchorBar: number; startBar: number; endBar: number }
  | { kind: "edge"; edge: "start" | "end"; startBar: number; endBar: number };

/**
 * Ruler gestures: click locates, drag defines a cycle range, dragging a
 * cycle edge trims it. The range in progress is a local overlay; the
 * transport commands go out on release.
 */
export function useRulerInteraction() {
  const store = useStore();
  const [overlay, setOverlay] = useState<RulerOverlay>({});
  const drag = useRef<Drag | null>(null);

  const localX = (e: ReactPointerEvent<HTMLElement>) =>
    e.clientX - e.currentTarget.getBoundingClientRect().left;

  const onPointerDown = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      const state = store.getState();
      const geo = laneGeometry(state);
      const x = localX(e);
      const t = state.transport;
      if (t.cycle) {
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
      const x = localX(e);
      const d = drag.current;
      if (!d) {
        const t = state.transport;
        const nearEdge =
          t.cycle &&
          (Math.abs(x - barToX(t.cycleStartBar, geo)) <= size.cycleGrip ||
            Math.abs(x - barToX(t.cycleEndBar, geo)) <= size.cycleGrip);
        e.currentTarget.style.cursor = nearEdge ? "ew-resize" : "default";
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
      setOverlay({});
      if (!d) return;
      const state = store.getState();
      if (d.kind === "pending") {
        const geo = laneGeometry(state);
        const raw = Math.max(0, xToBar(localX(e), geo));
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
