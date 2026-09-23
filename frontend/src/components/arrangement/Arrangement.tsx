import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent,
} from "react";
import { beatsToBars, commands } from "@ondera/core";
import { importAudioFiles } from "../../state/document";
import { useDispatch, useSession, useStore } from "../../state/session";
import { useCanvasSurface } from "../../canvas/surface";
import { drawRuler } from "../../canvas/ruler";
import {
  drawLanes,
  hitTestClip,
  laneGeometry,
  barToX,
} from "../../canvas/timeline";
import { useTimelineWheel } from "./useTimelineWheel";
import { useLaneInteraction } from "./useLaneInteraction";
import { useRulerInteraction } from "./useRulerInteraction";
import { TrackHeader } from "./TrackHeader";
import { CapsLabel } from "../primitives/CapsLabel";
import { Button } from "../primitives/Button";
import { InlineEdit } from "../primitives/InlineEdit";
import { PopupMenu, type MenuState } from "../menu/PopupMenu";
import { actionItem, separator, type MenuEntry } from "../../state/menus";
import {
  runAction,
  reportLaneViewportWidth,
  type ActionId,
} from "../../state/actions";
import { size } from "../../theme/tokens";
import styles from "./Arrangement.module.css";

/** Ruler row plus the scrolling track list: DOM headers on the left, one canvas for every lane. */
export function Arrangement() {
  return (
    <div className={styles.arrangement}>
      <RulerRow />
      <TrackList />
    </div>
  );
}

function RulerRow() {
  const store = useStore();
  const ruler = useRulerInteraction();
  const rulerRef = useCanvasSurface(
    useCallback(
      (ctx, w, h) => drawRuler(ctx, w, h, store.getState(), ruler.overlay),
      [store, ruler.overlay],
    ),
  );
  const wrapRef = useRef<HTMLDivElement>(null);
  useTimelineWheel(wrapRef);
  const [menu, setMenu] = useState<MenuState | null>(null);

  const openAddMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      items: [
        actionItem(store, "addAudioTrack"),
        actionItem(store, "addMidiTrack"),
      ],
    });
  };

  return (
    <div className={styles.rulerRow}>
      <div className={styles.rulerCorner}>
        <Button
          size="icon"
          className={styles.addTrack}
          title="Add track"
          onClick={openAddMenu}
        >
          +
        </Button>
        <CapsLabel>Tracks</CapsLabel>
      </div>
      <div
        ref={wrapRef}
        className={styles.rulerCanvasWrap}
        onPointerDown={ruler.onPointerDown}
        onPointerMove={ruler.onPointerMove}
        onPointerUp={ruler.onPointerUp}
        title="Click to locate · drag to set the cycle range"
      >
        <canvas ref={rulerRef} className={styles.canvas} />
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

function TrackList() {
  const store = useStore();
  const dispatch = useDispatch();
  const tracks = useSession((s) => s.tracks);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState({ top: 0, height: 1 });
  useEffect(() => {
    const el = scrollerRef.current!;
    const update = () =>
      setViewport({ top: el.scrollTop, height: el.clientHeight });
    const observer = new ResizeObserver(update);
    observer.observe(el);
    el.addEventListener("scroll", update, { passive: true });
    update();
    return () => {
      observer.disconnect();
      el.removeEventListener("scroll", update);
    };
  }, []);
  const lanesRef = useRef<HTMLDivElement>(null);
  const lanes = useLaneInteraction();
  const canvasRef = useCanvasSurface(
    useCallback(
      (ctx, w, h) => {
        ctx.save();
        ctx.translate(0, -viewport.top);
        drawLanes(
          ctx,
          w,
          h + viewport.top,
          store.getState(),
          lanes.overlay,
          viewport.top,
        );
        ctx.restore();
      },
      [store, lanes.overlay, viewport.top],
    ),
  );
  useTimelineWheel(lanesRef);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);

  // Report the viewport width for zoom actions, and page-flip the view to keep the playhead visible.
  useEffect(() => {
    const el = lanesRef.current;
    if (!el) return;
    // The host keeps the width too: view.fit and scripts need it.
    let reported = 0;
    const report = () => {
      const width = Math.round(el.clientWidth);
      reportLaneViewportWidth(width);
      if (width >= 50 && width !== reported) {
        reported = width;
        store.fire("view.set", { laneWidth: width });
      }
    };
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    const off = store.subscribe(() => {
      const s = store.getState();
      if (!s.transport.playing || !s.view.followPlayhead) return;
      const geo = laneGeometry(s);
      const w = el.clientWidth;
      const bar = beatsToBars(
        s.transport.positionBeats,
        s.transport.timeSignature,
      );
      const px = barToX(bar, geo);
      if (px > w - size.clipEdgeGrip * 2 || px < 0) {
        store.dispatch(
          commands.view.scrollTo({
            bar: Math.max(0, bar - (w * 0.08) / geo.ppb),
          }),
        );
      }
    });
    return () => {
      ro.disconnect();
      off();
    };
  }, [store]);

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const state = store.getState();
    const r = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - r.left;
    const y = e.clientY - r.top;
    const clip = hitTestClip(state, x, y);
    if (clip) {
      dispatch(commands.clip.select({ clipId: clip.id }));
      // What the menu offers depends on the clicked clip being selected; the select above
      // reaches the store later.
      const picked = {
        ...state,
        view: {
          ...state.view,
          selectedTrackId: clip.trackId,
          selectedClipId: clip.id,
          selectedNoteId: null,
        },
      };
      const item = (id: ActionId, label?: string) =>
        actionItem(store, id, label, picked);
      const items: MenuEntry[] = [
        item("openInEditor"),
        { label: "Rename…", onSelect: () => setRenaming(clip.id) },
        separator,
        item("cut"),
        item("copy"),
        item("duplicateClip"),
        item("splitAtPlayhead"),
        separator,
        item("deleteSelection", "Delete Clip"),
        separator,
        item("askAgent", "Ask Agent About This Region…"),
      ];
      setMenu({ x: e.clientX, y: e.clientY, items });
      return;
    }
    const track = state.tracks[Math.floor(y / size.trackRow)];
    if (track) dispatch(commands.track.select({ trackId: track.id }));
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        actionItem(store, "paste"),
        separator,
        actionItem(store, "addAudioTrack"),
        actionItem(store, "addMidiTrack"),
        separator,
        {
          label: "Import Audio…",
          onSelect: () => void importAudioFiles(store),
        },
        separator,
        actionItem(store, "askAgent", "Ask Agent…"),
      ],
    });
  };

  const renamingClip = renaming
    ? tracks && store.getState().clips.find((c) => c.id === renaming)
    : undefined;
  const renameStyle = (() => {
    if (!renamingClip) return undefined;
    const s = store.getState();
    const geo = laneGeometry(s);
    const row = s.tracks.findIndex((t) => t.id === renamingClip.trackId);
    return {
      left: barToX(renamingClip.startBar, geo) + 2,
      top: row * size.trackRow + size.clipInset - 2,
      width: Math.max(80, renamingClip.lengthBars * geo.ppb - 4),
    };
  })();

  return (
    <div ref={scrollerRef} className={styles.scroller}>
      {tracks.length === 0 && (
        <div className={styles.emptyProject}>
          <h2>Your next track starts here</h2>
          <p>
            Add an instrument and draw a region, import a recording, or explore
            the demo.
          </p>
          <div>
            <Button
              size="auto"
              onClick={() => runAction(store, "addMidiTrack")}
            >
              Add an instrument
            </Button>
            <Button
              size="auto"
              onClick={() => store.fire("web.file", { action: "import" })}
            >
              Import audio…
            </Button>
            <Button
              size="auto"
              onClick={() => store.fire("web.file", { action: "demo" })}
            >
              Open demo
            </Button>
          </div>
          <button
            className="m-button"
            onClick={() => store.fire("ui.showPanel", { panel: "agent" })}
          >
            Make something with an agent →
          </button>
        </div>
      )}
      <div className={styles.content}>
        <div className={styles.headers}>
          {tracks.map((t) => (
            <TrackHeader key={t.id} track={t} />
          ))}
          <div className={styles.headersEmpty} />
        </div>
        <div
          ref={lanesRef}
          className={styles.lanes}
          onPointerDown={lanes.onPointerDown}
          onPointerMove={lanes.onPointerMove}
          onPointerUp={lanes.onPointerUp}
          onPointerCancel={lanes.onPointerCancel}
          onLostPointerCapture={lanes.onPointerCancel}
          onClick={lanes.onClick}
          onDoubleClick={lanes.onDoubleClick}
          onContextMenu={onContextMenu}
        >
          <canvas
            ref={canvasRef}
            className={styles.canvas}
            style={{ top: viewport.top, height: viewport.height }}
          />
          <AgentChips />
          {renamingClip && renameStyle && (
            <InlineEdit
              className={styles.renameInput}
              style={renameStyle}
              value={renamingClip.name}
              onCommit={(name) => {
                dispatch(
                  commands.clip.rename({ clipId: renamingClip.id, name }),
                );
                setRenaming(null);
              }}
              onCancel={() => setRenaming(null)}
            />
          )}
        </div>
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

/** Frosted chip floating over the lane an agent is editing, anchored to the end of its clip. */
function AgentChips() {
  const tracks = useSession((s) => s.tracks);
  const clips = useSession((s) => s.clips);
  const current = useSession((s) => s.agent.current);
  const ppb = useSession((s) => s.view.pixelsPerBar);
  const scrollBars = useSession((s) => s.view.scrollBars);
  if (!current) return null;
  const geo = { ppb, scrollBars, rowHeight: size.trackRow };
  return (
    <>
      {tracks.map((t, i) => {
        if (!t.agentActive) return null;
        const clip = clips.find((c) => c.trackId === t.id && c.agent);
        const anchorBar = clip ? clip.startBar + clip.lengthBars : 0;
        const left = barToX(anchorBar, geo) + 6;
        return (
          <div
            key={t.id}
            className={`${styles.agentChip} m-glass-agent`}
            style={{ left, top: i * size.trackRow - 11 }}
          >
            <span className={`${styles.agentDot} m-accent-dot`} />
            Agent · {current.summary}
          </div>
        );
      })}
    </>
  );
}
