import { commands, type BrowserTab } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
import { SegmentedControl } from '../primitives/SegmentedControl';
import { Button } from '../primitives/Button';
import { CapsLabel } from '../primitives/CapsLabel';
import { PlaySmallIcon, SearchIcon } from '../primitives/Icons';
import styles from './BrowserPanel.module.css';

const TABS: { id: BrowserTab; label: string }[] = [
  { id: 'instruments', label: 'Instr' },
  { id: 'loops', label: 'Loops' },
  { id: 'plugins', label: 'Plugins' },
  { id: 'files', label: 'Files' },
];

export function BrowserPanel() {
  const dispatch = useDispatch();
  const tab = useSession((s) => s.view.browserTab);
  const groups = useSession((s) => s.browser);

  return (
    <div className={styles.panel}>
      <div className={styles.tabs}>
        <SegmentedControl items={TABS} value={tab} stretch onChange={(id) => dispatch(commands.view.setBrowserTab({ tab: id }))} />
      </div>
      <div className={`${styles.search} m-groove-alt`}>
        <SearchIcon />
        Search instruments
      </div>
      <div className={styles.list}>
        {groups.map((g) => (
          <div key={g.name}>
            <CapsLabel style={{ padding: '8px 8px 4px' }}>{g.name}</CapsLabel>
            {g.items.map((it) => (
              <div key={it.name} className={`${styles.item} ${it.highlighted ? styles.highlighted : ''}`}>
                <span className={`${styles.dot} m-swatch`} style={{ background: it.color ?? 'var(--color-neutral-dot)' }} />
                {it.name}
                <span className={styles.meta}>{it.meta}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className={styles.footer}>
        <Button size="icon" className={styles.preview}>
          <PlaySmallIcon />
        </Button>
        <span>Preview</span>
        <span className={styles.previewName}>E-Piano Mk I</span>
      </div>
    </div>
  );
}
