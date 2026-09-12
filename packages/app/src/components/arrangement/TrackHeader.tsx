import { commands, type Track } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
import { Button } from '../primitives/Button';
import { HSlider } from '../primitives/HSlider';
import { Knob } from '../primitives/Knob';
import { RecordSmallIcon } from '../primitives/Icons';
import styles from './TrackHeader.module.css';

/** Degrees of knob rotation per pan unit; ±100 maps to ±135°. */
const PAN_DEGREES = 1.35;

export function TrackHeader({ track }: { track: Track }) {
  const dispatch = useDispatch();
  const selected = useSession((s) => s.view.selectedTrackId === track.id);
  const state = selected ? styles.selected : track.agentActive ? styles.agent : '';

  return (
    <div className={`${styles.header} ${state}`} onClick={() => dispatch(commands.track.select({ trackId: track.id }))}>
      <div className={styles.strip} style={{ background: track.color }} />
      <div className={styles.nameRow}>
        <span className={styles.name}>{track.name}</span>
        <span className={styles.kind}>{track.kind === 'audio' ? 'AUD' : 'MIDI'}</span>
        {track.agentActive && <span className={`${styles.agentDot} m-accent-dot`} />}
      </div>
      <div className={styles.controls} onClick={(e) => e.stopPropagation()}>
        <div className={styles.buttons}>
          <Button size="sm" pressed={track.mute} onClick={() => dispatch(commands.track.setMute({ trackId: track.id, muted: !track.mute }))}>
            M
          </Button>
          <Button size="sm" pressed={track.solo} onClick={() => dispatch(commands.track.setSolo({ trackId: track.id, solo: !track.solo }))}>
            S
          </Button>
          <Button size="sm" lit={track.armed} onClick={() => dispatch(commands.track.setArmed({ trackId: track.id, armed: !track.armed }))}>
            <RecordSmallIcon />
          </Button>
        </div>
        <HSlider value={track.volume} onChange={(v) => dispatch(commands.track.setVolume({ trackId: track.id, volume: v }))} />
        <Knob angle={track.pan * PAN_DEGREES} title={`Pan ${track.pan}`} />
      </div>
    </div>
  );
}
