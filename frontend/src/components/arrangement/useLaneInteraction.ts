import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  barsToSeconds,
  clipEnvelope,
  commands,
  snapBars,
  snapStepBars,
  type Clip,
  type Session,
} from "@ondera/core";
import { useSession, useStore } from "../../state/session";
import { newId } from "../../state/ids";
import {
  barToX,
  clipEdgeAt,
  fadeHandleAt,
  hitTestClip,
  laneGeometry,
  pixelsPerSecond,
  xToBar,
  type LaneOverlay,
} from "../../canvas/timeline";
import { size } from "../../theme/tokens";

const DRAG_THRESHOLD_PX = 3;

type Drag =
  | {
      kind: "move";
      clip: Clip;
      grabBar: number;
      startX: number;
      startY: number;
      startBar: number;
      trackIndex: number;
      moved: boolean;
    }
  | {
      kind: "resize";
      clip: Clip;
      edge: "start" | "end";
      startBar: number;
      lengthBars: number;
    }
  | {
      kind: "fade";
      clip: Clip;
      handle: "in" | "out";
      fadeIn: number;
      fadeOut: number;
    }
  | {
      kind: "pencil";
      trackIndex: number;
      anchorBar: number;
      startBar: number;
      lengthBars: number;
    };

interface Point {
  x: number;
  y: number;
}

/**
 * Pointer handling for the lanes canvas. Drags are local React state (the
 * ghost you see while dragging); the command is dispatched once on release
 * so each gesture is one undo step.
 */
export function useLaneInteraction() {
  const store = useStore();
  const [overlay, setOverlay] = useState<LaneOverlay>({});
  const drag = useRef<Drag | null>(null);
  const suppressClick = useRef(false);
  const clips = useSession((s) => s.clips);
  useEffect(() => {
    if (!drag.current) setOverlay((o) => (o.fade ? {} : o));
  }, [clips]);

  const point = (e: ReactPointerEvent<HTMLElement>): Point => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const snap = (state: Session, bar: number, fine: boolean) =>
    fine
      ? bar
      : snapBars(
          bar,
          state.transport.snapDivision,
          state.transport.timeSignature,
        );
  const step = (state: Session) =>
    snapStepBars(state.transport.snapDivision, state.transport.timeSignature);

  const cursorFor = (state: Session, p: Point): string => {
    const tool = state.view.arrangeTool;
    const clip = hitTestClip(state, p.x, p.y);
    if (tool === "scissors") return clip ? "col-resize" : "default";
    if (tool === "pencil") return clip ? "default" : "crosshair";
    if (clip && fadeHandleAt(state, clip, p.x, p.y)) return "ew-resize";
    if (clip && clipEdgeAt(state, clip, p.x)) return "ew-resize";
    return "default";
  };

  const onPointerDown = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      const state = store.getState();
      const p = point(e);
      const geo = laneGeometry(state);
      const trackIndex = Math.floor(p.y / geo.rowHeight);
      const track = state.tracks[trackIndex];
      const clip = hitTestClip(state, p.x, p.y);
      const bar = xToBar(p.x, geo);
      const tool = state.view.arrangeTool;

      if (tool === "scissors") {
        if (clip) {
          const at = snap(state, bar, e.altKey);
          if (at > clip.startBar && at < clip.startBar + clip.lengthBars) {
            store.dispatch(
              commands.clip.split({
                clipId: clip.id,
                atBar: at,
                newClipId: newId("clip"),
              }),
            );
          }
        }
        suppressClick.current = true;
        return;
      }

      if (tool === "pencil" && !clip && track) {
        const anchor = Math.max(0, snap(state, bar, e.altKey));
        drag.current = {
          kind: "pencil",
          trackIndex,
          anchorBar: anchor,
          startBar: anchor,
          lengthBars: 0,
        };
        e.currentTarget.setPointerCapture(e.pointerId);
        suppressClick.current = true;
        return;
      }

      if (clip) {
        store.dispatch(commands.clip.select({ clipId: clip.id }));
        const handle = fadeHandleAt(state, clip, p.x, p.y);
        const edge = clipEdgeAt(state, clip, p.x);
        if (handle && clip.data.kind === "audio") {
          const env = clipEnvelope(clip.data);
          drag.current = {
            kind: "fade",
            clip,
            handle,
            fadeIn: env.fadeIn,
            fadeOut: env.fadeOut,
          };
        } else if (edge) {
          drag.current = {
            kind: "resize",
            clip,
            edge,
            startBar: clip.startBar,
            lengthBars: clip.lengthBars,
          };
        } else {
          drag.current = {
            kind: "move",
            clip,
            grabBar: bar - clip.startBar,
            startX: p.x,
            startY: p.y,
            startBar: clip.startBar,
            trackIndex,
            moved: false,
          };
        }
        e.currentTarget.setPointerCapture(e.pointerId);
        suppressClick.current = true;
        return;
      }
      suppressClick.current = false;
    },
    [store],
  );

  const onPointerMove = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      const state = store.getState();
      const p = point(e);
      const d = drag.current;
      if (!d) {
        e.currentTarget.style.cursor = cursorFor(state, p);
        if (state.view.arrangeTool === "scissors") {
          const clip = hitTestClip(state, p.x, p.y);
          const geo = laneGeometry(state);
          setOverlay(
            clip
              ? {
                  split: {
                    trackIndex: Math.floor(p.y / geo.rowHeight),
                    bar: snap(state, xToBar(p.x, geo), e.altKey),
                  },
                }
              : {},
          );
        } else if (overlay.split) {
          setOverlay({});
        }
        return;
      }
      const geo = laneGeometry(state);
      const bar = xToBar(p.x, geo);
      const minLen = step(state);
      if (d.kind === "move") {
        if (
          !d.moved &&
          Math.hypot(p.x - d.startX, p.y - d.startY) < DRAG_THRESHOLD_PX
        )
          return;
        d.moved = true;
        e.currentTarget.style.cursor = "grabbing";
        const rawStart = Math.max(0, bar - d.grabBar);
        d.startBar = e.altKey
          ? rawStart
          : Math.max(
              0,
              snapBars(
                rawStart,
                state.transport.snapDivision,
                state.transport.timeSignature,
              ),
            );
        const row = Math.max(
          0,
          Math.min(state.tracks.length - 1, Math.floor(p.y / geo.rowHeight)),
        );
        const target = state.tracks[row];
        if (target && target.kind === d.clip.data.kind) d.trackIndex = row;
        setOverlay({
          ghost: {
            trackIndex: d.trackIndex,
            startBar: d.startBar,
            lengthBars: d.clip.lengthBars,
          },
        });
      } else if (d.kind === "resize") {
        const origEnd = d.clip.startBar + d.clip.lengthBars;
        if (d.edge === "start") {
          const ns = Math.min(
            origEnd - minLen,
            Math.max(0, snap(state, bar, e.altKey)),
          );
          d.startBar = ns;
          d.lengthBars = origEnd - ns;
        } else {
          const ne = Math.max(
            d.clip.startBar + minLen,
            snap(state, bar, e.altKey),
          );
          d.lengthBars = ne - d.clip.startBar;
        }
        const row = state.tracks.findIndex((t) => t.id === d.clip.trackId);
        setOverlay({
          ghost: {
            trackIndex: row,
            startBar: d.startBar,
            lengthBars: d.lengthBars,
          },
        });
      } else if (d.kind === "fade") {
        // Fades are seconds of audio: free, not on the bar grid, and never past the other fade.
        const { tempo, timeSignature } = state.transport;
        const length = barsToSeconds(d.clip.lengthBars, tempo, timeSignature);
        const pps = pixelsPerSecond(state);
        const x0 = barToX(d.clip.startBar, geo);
        const x1 = barToX(d.clip.startBar + d.clip.lengthBars, geo);
        const ms = (v: number) => Math.round(v * 1000) / 1000;
        if (d.handle === "in")
          d.fadeIn = ms(
            Math.max(0, Math.min(length - d.fadeOut, (p.x - x0) / pps)),
          );
        else
          d.fadeOut = ms(
            Math.max(0, Math.min(length - d.fadeIn, (x1 - p.x) / pps)),
          );
        e.currentTarget.style.cursor = "ew-resize";
        setOverlay({
          fade: { clipId: d.clip.id, fadeIn: d.fadeIn, fadeOut: d.fadeOut },
        });
      } else {
        const b = Math.max(0, snap(state, bar, e.altKey));
        d.startBar = Math.min(d.anchorBar, b);
        d.lengthBars = Math.abs(b - d.anchorBar);
        setOverlay({
          pencil: {
            trackIndex: d.trackIndex,
            startBar: d.startBar,
            lengthBars: Math.max(d.lengthBars, minLen),
          },
        });
      }
    },
    [store, overlay.split],
  );

  const onPointerUp = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      const d = drag.current;
      drag.current = null;
      // A fade stays where it was dropped until the host's document has it.
      if (d?.kind !== "fade") setOverlay({});
      e.currentTarget.style.cursor = "default";
      if (!d) return;
      const state = store.getState();
      if (d.kind === "move") {
        if (!d.moved) return;
        const target = state.tracks[d.trackIndex];
        if (
          d.startBar !== d.clip.startBar ||
          (target && target.id !== d.clip.trackId)
        ) {
          store.dispatch(
            commands.clip.move({
              clipId: d.clip.id,
              startBar: d.startBar,
              ...(target ? { trackId: target.id } : {}),
            }),
          );
        }
      } else if (d.kind === "resize") {
        if (
          d.startBar !== d.clip.startBar ||
          d.lengthBars !== d.clip.lengthBars
        ) {
          store.dispatch(
            commands.clip.resize({
              clipId: d.clip.id,
              startBar: d.startBar,
              lengthBars: d.lengthBars,
            }),
          );
        }
      } else if (d.kind === "fade") {
        if (d.clip.data.kind !== "audio") return;
        const env = clipEnvelope(d.clip.data);
        if (d.fadeIn === env.fadeIn && d.fadeOut === env.fadeOut)
          setOverlay({});
        else
          store.dispatch(
            commands.clip.setFades({
              clipId: d.clip.id,
              fadeInSeconds: d.fadeIn,
              fadeOutSeconds: d.fadeOut,
            }),
          );
        setTimeout(() => {
          if (!drag.current) setOverlay((o) => (o.fade ? {} : o));
        }, 1000);
      } else {
        const track = state.tracks[d.trackIndex];
        if (!track) return;
        const len = Math.max(
          d.lengthBars,
          d.lengthBars === 0 ? 1 : step(state),
        );
        if (track.kind === "midi") {
          store.dispatch(
            commands.clip.create({
              clipId: newId("clip"),
              trackId: track.id,
              startBar: d.startBar,
              lengthBars: len,
            }),
          );
        } else {
          // Audio needs a source: pencil on an audio track just selects it. Import or record instead.
          store.dispatch(commands.track.select({ trackId: track.id }));
        }
      }
    },
    [store],
  );

  /**
   * The system took the pointer (a gesture, a dialog, a lost capture): drop the drag and its
   * ghost. Left alone, the next release would commit a move nobody made.
   */
  const onPointerCancel = useCallback(
    (e: ReactPointerEvent<HTMLDivElement>) => {
      if (!drag.current) return;
      drag.current = null;
      setOverlay({});
      e.currentTarget.style.cursor = "default";
    },
    [],
  );

  /** Plain click on empty lane: clear clip selection and select the track. */
  const onClick = useCallback(
    (
      e: ReactPointerEvent<HTMLDivElement> | React.MouseEvent<HTMLDivElement>,
    ) => {
      if (suppressClick.current) {
        suppressClick.current = false;
        return;
      }
      const state = store.getState();
      const r = e.currentTarget.getBoundingClientRect();
      const y = e.clientY - r.top;
      if (state.view.selectedClipId)
        store.dispatch(commands.clip.clearSelection({}));
      const track = state.tracks[Math.floor(y / size.trackRow)];
      if (track && track.id !== state.view.selectedTrackId)
        store.dispatch(commands.track.select({ trackId: track.id }));
    },
    [store],
  );

  const onDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const state = store.getState();
      const r = e.currentTarget.getBoundingClientRect();
      const clip = hitTestClip(state, e.clientX - r.left, e.clientY - r.top);
      if (clip?.data.kind === "midi")
        store.dispatch(commands.view.setEditorClip({ clipId: clip.id }));
    },
    [store],
  );

  return {
    overlay,
    setOverlay,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onClick,
    onDoubleClick,
  };
}
