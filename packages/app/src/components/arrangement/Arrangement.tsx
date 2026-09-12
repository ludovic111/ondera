import { useCallback, useRef, type MouseEvent } from 'react';
import { barsToBeats, commands } from '@ondera/core';
import { useDispatch, useSession, useStore } from '../../state/session';
import { useCanvasSurface } from '../../canvas/surface';
import { drawRuler } from '../../canvas/ruler';
import { drawLanes, hitTestClip, laneGeometry, xToBar, barToX } from '../../canvas/timeline';
import { useTimelineWheel } from './useTimelineWheel';
import { TrackHeader } from './TrackHeader';
import { CapsLabel } from '../primitives/CapsLabel';
import { Button } from '../primitives/Button';
import { size } from '../../theme/tokens';
import styles from './Arrangement.module.css';

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
  const dispatch = useDispatch();
  const rulerRef = useCanvasSurface(useCallback((ctx, w, h) => drawRuler(ctx, w, h, store.getState()), [store]));
  const wrapRef = useRef<HTMLDivElement>(null);
  useTimelineWheel(wrapRef);

  const locate = (e: MouseEvent<HTMLDivElement>) => {
    const state = store.getState();
    const x = e.clientX - e.currentTarget.getBoundingClientRect().left;
    const bar = Math.max(0, xToBar(x, laneGeometry(state)));
    dispatch(commands.transport.setPosition({ beats: barsToBeats(bar, state.transport.timeSignature) }));
  };

  return (
    <div className={styles.rulerRow}>
      <div className={styles.rulerCorner}>
        <Button size="icon" className={styles.addTrack} title="Add track">
          +
        </Button>
        <CapsLabel>Tracks</CapsLabel>
      </div>
      <div ref={wrapRef} className={styles.rulerCanvasWrap} onClick={locate}>
        <canvas ref={rulerRef} className={styles.canvas} />
      </div>
    </div>
  );
}

function TrackList() {
  const store = useStore();
  const dispatch = useDispatch();
  const tracks = useSession((s) => s.tracks);
  const lanesRef = useRef<HTMLDivElement>(null);
  const canvasRef = useCanvasSurface(useCallback((ctx, w, h) => drawLanes(ctx, w, h, store.getState()), [store]));
  useTimelineWheel(lanesRef);

  const onLaneClick = (e: MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const state = store.getState();
    const clip = hitTestClip(state, x, y);
    if (clip) {
      dispatch(commands.clip.select({ clipId: clip.id }));
      return;
    }
    dispatch(commands.clip.clearSelection({}));
    const track = state.tracks[Math.floor(y / size.trackRow)];
    if (track) dispatch(commands.track.select({ trackId: track.id }));
  };

  return (
    <div className={styles.scroller}>
      <div className={styles.content}>
        <div className={styles.headers}>
          {tracks.map((t) => (
            <TrackHeader key={t.id} track={t} />
          ))}
          <div className={styles.headersEmpty} />
        </div>
        <div ref={lanesRef} className={styles.lanes} onClick={onLaneClick}>
          <canvas ref={canvasRef} className={styles.canvas} />
          <AgentChips />
        </div>
      </div>
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
          <div key={t.id} className={`${styles.agentChip} m-glass-agent`} style={{ left, top: i * size.trackRow - 11 }}>
            <span className={`${styles.agentDot} m-accent-dot`} />
            Agent · {current.summary}
          </div>
        );
      })}
    </>
  );
}
