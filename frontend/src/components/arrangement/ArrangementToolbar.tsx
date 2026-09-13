import { useRef, type PointerEvent, type ReactNode } from "react";
import {
  commands,
  ZOOM_MAX_PX_PER_BAR,
  ZOOM_MIN_PX_PER_BAR,
  type ArrangeTool,
} from "@ondera/core";
import { useDispatch, useSession } from "../../state/session";
import { SegmentedControl } from "../primitives/SegmentedControl";
import { PencilIcon, PointerIcon, ScissorsIcon } from "../primitives/Icons";
import { size } from "../../theme/tokens";
import styles from "./ArrangementToolbar.module.css";

const TOOLS: { id: ArrangeTool; label: ReactNode; title: string }[] = [
  { id: "pointer", label: <PointerIcon />, title: "Pointer" },
  { id: "pencil", label: <PencilIcon />, title: "Pencil" },
  { id: "scissors", label: <ScissorsIcon />, title: "Scissors" },
];

const LOG_MIN = Math.log(ZOOM_MIN_PX_PER_BAR);
const LOG_MAX = Math.log(ZOOM_MAX_PX_PER_BAR);
const zoomToT = (ppb: number) =>
  (Math.log(ppb) - LOG_MIN) / (LOG_MAX - LOG_MIN);
const tToZoom = (t: number) => Math.exp(LOG_MIN + t * (LOG_MAX - LOG_MIN));

export function ArrangementToolbar() {
  const dispatch = useDispatch();
  const tool = useSession((s) => s.view.arrangeTool);
  const snap = useSession((s) => s.transport.snapDivision);
  const follow = useSession((s) => s.view.followPlayhead);
  const ppb = useSession((s) => s.view.pixelsPerBar);
  const cycleStart = useSession((s) => s.transport.cycleStartBar);
  const cycleEnd = useSession((s) => s.transport.cycleEndBar);
  const railRef = useRef<HTMLDivElement>(null);

  const zoomFromEvent = (e: PointerEvent) => {
    const rect = railRef.current?.getBoundingClientRect();
    if (!rect) return ppb;
    const travel = rect.width - size.zoomThumb;
    const t = Math.min(
      1,
      Math.max(0, (e.clientX - rect.left - size.zoomThumb / 2) / travel),
    );
    return tToZoom(t);
  };
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    dispatch(commands.view.setZoom({ pixelsPerBar: zoomFromEvent(e) }));
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    dispatch(commands.view.setZoom({ pixelsPerBar: zoomFromEvent(e) }));
  };

  return (
    <div className={styles.bar}>
      <SegmentedControl
        items={TOOLS}
        value={tool}
        icons
        onChange={(id) => dispatch(commands.view.setArrangeTool({ tool: id }))}
      />
      <div className={styles.info}>
        <span>
          <span className={styles.dim}>Grid</span> 1/{snap}
        </span>
        <span>
          <span className={styles.dim}>Cycle</span> {cycleStart + 1} –{" "}
          {cycleEnd + 1}
        </span>
        <button
          className="plain-control"
          aria-pressed={follow}
          onClick={() =>
            dispatch(commands.view.setFollowPlayhead({ enabled: !follow }))
          }
        >
          {follow ? "Follow" : "Follow off"}
        </button>
      </div>
      <div className={styles.zoom}>
        <span>Zoom</span>
        <div
          ref={railRef}
          className={`${styles.rail} m-groove-alt`}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
        >
          <div
            className={styles.thumb}
            style={{
              left: `calc(${zoomToT(ppb)} * (100% - var(--size-zoom-thumb)))`,
            }}
          />
        </div>
      </div>
    </div>
  );
}
