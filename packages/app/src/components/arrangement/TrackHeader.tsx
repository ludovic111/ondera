import { useState, type MouseEvent } from 'react';
import { commands, TRACK_PALETTE, type Track } from '@ondera/core';
import { useDispatch, useSession, useStore } from '../../state/session';
import { Button } from '../primitives/Button';
import { HSlider } from '../primitives/HSlider';
import { Knob } from '../primitives/Knob';
import { InlineEdit } from '../primitives/InlineEdit';
import { RecordSmallIcon } from '../primitives/Icons';
import { PopupMenu, type MenuState } from '../menu/PopupMenu';
import { actionItem, separator, type MenuEntry } from '../../state/menus';
import styles from './TrackHeader.module.css';

/** Degrees of knob rotation per pan unit; ±100 maps to ±135°. */
const PAN_DEGREES = 1.35;

const formatPan = (pan: number) => (pan === 0 ? 'C' : pan < 0 ? `L ${-Math.round(pan)}` : `R ${Math.round(pan)}`);

export function TrackHeader({ track }: { track: Track }) {
  const store = useStore();
  const dispatch = useDispatch();
  const selected = useSession((s) => s.view.selectedTrackId === track.id);
  const index = useSession((s) => s.tracks.findIndex((t) => t.id === track.id));
  const count = useSession((s) => s.tracks.length);
  const state = selected ? styles.selected : track.agentActive ? styles.agent : '';
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [renaming, setRenaming] = useState(false);

  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    dispatch(commands.track.select({ trackId: track.id }));
    const colors: MenuEntry[] = Object.entries(TRACK_PALETTE).map(([name, color]) => ({
      label: `Colour · ${name}`,
      checked: track.color === color,
      onSelect: () => dispatch(commands.track.setColor({ trackId: track.id, color })),
    }));
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: 'Rename…', onSelect: () => setRenaming(true) },
        actionItem(store, 'muteSelectedTrack'),
        actionItem(store, 'soloSelectedTrack'),
        actionItem(store, 'armSelectedTrack'),
        separator,
        { label: 'Move Up', disabled: index === 0, onSelect: () => dispatch(commands.track.move({ trackId: track.id, index: index - 1 })) },
        { label: 'Move Down', disabled: index >= count - 1, onSelect: () => dispatch(commands.track.move({ trackId: track.id, index: index + 1 })) },
        separator,
        ...colors,
        separator,
        actionItem(store, 'addAudioTrack'),
        actionItem(store, 'addMidiTrack'),
        actionItem(store, 'removeSelectedTrack'),
      ],
    });
  };

  return (
    <div
      className={`${styles.header} ${state}`}
      onClick={() => dispatch(commands.track.select({ trackId: track.id }))}
      onContextMenu={onContextMenu}
    >
      <div className={styles.strip} style={{ background: track.color }} />
      <div className={styles.nameRow}>
        {renaming ? (
          <InlineEdit
            className={styles.nameInput}
            value={track.name}
            onCommit={(name) => {
              dispatch(commands.track.rename({ trackId: track.id, name }));
              setRenaming(false);
            }}
            onCancel={() => setRenaming(false)}
          />
        ) : (
          <span className={styles.name} onDoubleClick={() => setRenaming(true)} title="Double-click to rename">
            {track.name}
          </span>
        )}
        <span className={styles.kind}>{track.kind === 'audio' ? 'AUD' : 'MIDI'}</span>
        {track.agentActive && <span className={`${styles.agentDot} m-accent-dot`} />}
      </div>
      <div className={styles.controls} onClick={(e) => e.stopPropagation()}>
        <div className={styles.buttons}>
          <Button size="sm" title="Mute" pressed={track.mute} onClick={() => dispatch(commands.track.setMute({ trackId: track.id, muted: !track.mute }))}>
            M
          </Button>
          <Button size="sm" title="Solo" pressed={track.solo} onClick={() => dispatch(commands.track.setSolo({ trackId: track.id, solo: !track.solo }))}>
            S
          </Button>
          <Button size="sm" title="Record arm" lit={track.armed} onClick={() => dispatch(commands.track.setArmed({ trackId: track.id, armed: !track.armed }))}>
            <RecordSmallIcon />
          </Button>
        </div>
        <HSlider
          value={track.volume}
          title={`Volume ${Math.round(track.volume * 100)}%`}
          onChange={(v) => dispatch(commands.track.setVolume({ trackId: track.id, volume: v }))}
        />
        <Knob
          angle={track.pan * PAN_DEGREES}
          title={`Pan ${formatPan(track.pan)} · drag, double-click to centre`}
          value={track.pan}
          min={-100}
          max={100}
          defaultValue={0}
          onChange={(pan) => dispatch(commands.track.setPan({ trackId: track.id, pan: Math.round(pan) }))}
        />
      </div>
      {menu && <PopupMenu items={menu.items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
    </div>
  );
}
