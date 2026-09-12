import { useCallback } from 'react';
import { commands, type EditorMode } from '@ondera/core';
import { useDispatch, useSession, useStore } from '../../state/session';
import { useCanvasSurface } from '../../canvas/surface';
import { drawPianoRoll, editorClip, editorLowPitch, isBlackKey, noteLabel } from '../../canvas/pianoRoll';
import { SegmentedControl } from '../primitives/SegmentedControl';
import { size } from '../../theme/tokens';
import styles from './EditorPane.module.css';

const MODES: { id: EditorMode; label: string }[] = [
  { id: 'pianoRoll', label: 'Piano Roll' },
  { id: 'score', label: 'Score' },
  { id: 'step', label: 'Step' },
];

export function EditorPane() {
  const dispatch = useDispatch();
  const store = useStore();
  const mode = useSession((s) => s.view.editorMode);
  const clip = useSession((s) => editorClip(s));
  const track = useSession((s) => (clip ? (s.tracks.find((t) => t.id === clip.trackId) ?? null) : null));
  const key = useSession((s) => s.transport.key);
  const snap = useSession((s) => s.transport.snapDivision);
  const canvasRef = useCanvasSurface(useCallback((ctx, w, h) => drawPianoRoll(ctx, w, h, store.getState()), [store]));

  const low = editorLowPitch(clip);
  const rows = Array.from({ length: size.keyRows }, (_, r) => low + (size.keyRows - 1 - r));

  return (
    <div className={styles.pane}>
      <div className={styles.header}>
        <SegmentedControl items={MODES} value={mode} onChange={(id) => dispatch(commands.view.setEditorMode({ mode: id }))} />
        {clip && track && (
          <div className={styles.clipTitle}>
            <span className={`${styles.swatch} m-swatch`} style={{ background: track.color }} />
            {clip.name}
            <span className={styles.range}>
              · bars {clip.startBar + 1} – {clip.startBar + clip.lengthBars}
            </span>
          </div>
        )}
        <div className={styles.params}>
          <span>
            <span className={styles.dim}>Quantize</span> 1/{snap}
          </span>
          <span>
            <span className={styles.dim}>Velocity</span> 96
          </span>
          <span>
            <span className={styles.dim}>Scale</span> {key.replace('min', 'minor')}
          </span>
        </div>
      </div>
      <div className={styles.body}>
        <div className={styles.keys}>
          <div className={styles.keysHeader} />
          {rows.map((pitch) => {
            const black = isBlackKey(pitch);
            return (
              <div key={pitch} className={`${styles.key} ${black ? styles.blackKey : styles.whiteKey}`}>
                {black ? '' : noteLabel(pitch)}
              </div>
            );
          })}
        </div>
        <div className={styles.grid}>
          <canvas ref={canvasRef} className={styles.canvas} />
        </div>
      </div>
    </div>
  );
}
