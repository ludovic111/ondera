import { useState } from 'react';
import { commands, secondsToBars, type BrowserItem, type BrowserTab } from '@ondera/core';
import { useDispatch, useSession, useStore } from '../../state/session';
import { newId } from '../../state/ids';
import { importAudioFiles, openSession } from '../../state/document';
import { SegmentedControl } from '../primitives/SegmentedControl';
import { Button } from '../primitives/Button';
import { CapsLabel } from '../primitives/CapsLabel';
import { PlaySmallIcon, SearchIcon } from '../primitives/Icons';
import { engine } from '../../audio/instance';
import { INSTRUMENT_NAMES } from '../../audio/instruments';
import { isEffectName } from '../../audio/effects';
import { loopNotes } from '../../audio/loops';
import styles from './BrowserPanel.module.css';

const TABS: { id: BrowserTab; label: string }[] = [
  { id: 'instruments', label: 'Instr' },
  { id: 'loops', label: 'Loops' },
  { id: 'plugins', label: 'Plugins' },
  { id: 'files', label: 'Files' },
];

const PLACEHOLDER: Record<BrowserTab, string> = {
  instruments: 'Search instruments',
  loops: 'Search loops',
  plugins: 'Search plugins',
  files: 'Search files',
};

const HINT: Record<BrowserTab, string> = {
  instruments: 'Double-click: load on the selected MIDI track, or add one',
  loops: 'Double-click: add a MIDI loop at the playhead',
  plugins: 'Double-click: insert on the selected track',
  files: 'Double-click: open or import',
};

export function BrowserPanel() {
  const store = useStore();
  const dispatch = useDispatch();
  const tab = useSession((s) => s.view.browserTab);
  const groups = useSession((s) => s.browser[s.view.browserTab]);
  const selection = useSession((s) => s.view.browserSelection);
  const [query, setQuery] = useState('');

  const q = query.trim().toLowerCase();
  const visible = groups
    .map((g) => ({ ...g, items: q ? g.items.filter((it) => it.name.toLowerCase().includes(q) || it.meta.toLowerCase().includes(q)) : g.items }))
    .filter((g) => g.items.length > 0);

  const activate = (item: BrowserItem) => {
    const s = store.getState();
    const selected = s.tracks.find((t) => t.id === s.view.selectedTrackId);
    switch (tab) {
      case 'instruments': {
        if (!INSTRUMENT_NAMES.includes(item.name)) return;
        let trackId = selected?.kind === 'midi' ? selected.id : null;
        if (!trackId) {
          trackId = newId('track');
          dispatch(commands.track.add({ trackId, kind: 'midi', name: item.name, ...(item.color ? { color: item.color } : {}) }));
        }
        dispatch(commands.strip.setInstrument({ trackId, instrument: item.name }));
        engine.previewNote(trackId, 60, 90);
        return;
      }
      case 'loops': {
        const loop = loopNotes(item.name);
        if (!loop) return;
        let trackId = selected?.kind === 'midi' ? selected.id : null;
        if (!trackId) {
          trackId = newId('track');
          dispatch(commands.track.add({ trackId, kind: 'midi', name: item.name, ...(item.color ? { color: item.color } : {}) }));
          dispatch(commands.strip.setInstrument({ trackId, instrument: loop.instrument }));
        }
        const startBar = Math.round(s.transport.positionBeats / (4 * (s.transport.timeSignature.numerator / s.transport.timeSignature.denominator)));
        const clipId = newId('clip');
        dispatch(commands.clip.create({ clipId, trackId, startBar, lengthBars: loop.bars, name: item.name }));
        for (const n of loop.notes) {
          dispatch(commands.note.add({ clipId, noteId: newId('note'), start: n.start, length: n.length, pitch: n.pitch, velocity: n.velocity }));
        }
        return;
      }
      case 'plugins': {
        if (!selected || !isEffectName(item.name)) return;
        const strip = s.strips[selected.id];
        const slot = strip ? strip.inserts.findIndex((i) => i.state === 'empty') : 0;
        dispatch(commands.strip.setInsert({ trackId: selected.id, slotIndex: slot < 0 ? strip!.inserts.length - 1 : slot, name: item.name }));
        return;
      }
      case 'files': {
        if (item.name.endsWith('.ondera')) void openSession(store);
        else void importAudioFiles(store);
        return;
      }
    }
  };

  const preview = () => {
    const s = store.getState();
    const track = s.tracks.find((t) => t.id === s.view.selectedTrackId && t.kind === 'midi') ?? s.tracks.find((t) => t.kind === 'midi');
    if (track) {
      engine.previewNote(track.id, 60, 90);
      setTimeout(() => engine.previewNote(track.id, 64, 80), 150);
      setTimeout(() => engine.previewNote(track.id, 67, 80), 300);
    }
  };

  return (
    <div className={styles.panel}>
      <div className={styles.tabs}>
        <SegmentedControl
          items={TABS}
          value={tab}
          stretch
          onChange={(id) => {
            dispatch(commands.view.setBrowserTab({ tab: id }));
            setQuery('');
          }}
        />
      </div>
      <div className={`${styles.search} m-groove-alt`}>
        <SearchIcon />
        <input className={styles.searchInput} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={PLACEHOLDER[tab]} spellCheck={false} />
      </div>
      <div className={styles.list} title={HINT[tab]}>
        {visible.length === 0 && <div className={styles.empty}>No matches</div>}
        {visible.map((g) => (
          <div key={g.name}>
            <CapsLabel style={{ padding: '8px 8px 4px' }}>{g.name}</CapsLabel>
            {g.items.map((it) => (
              <div
                key={it.name}
                className={`${styles.item} ${it.name === selection ? styles.highlighted : ''}`}
                onClick={() => dispatch(commands.view.setBrowserSelection({ name: it.name }))}
                onDoubleClick={() => activate(it)}
              >
                <span className={`${styles.dot} m-swatch`} style={{ background: it.color ?? 'var(--color-neutral-dot)' }} />
                {it.name}
                <span className={styles.meta}>{it.meta}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className={styles.footer}>
        <Button size="icon" className={styles.preview} onClick={preview} title="Audition the selected track's instrument">
          <PlaySmallIcon />
        </Button>
        <span>Preview</span>
        <span className={styles.previewName}>{selection ?? '—'}</span>
      </div>
    </div>
  );
}

export { secondsToBars };
