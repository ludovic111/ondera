import { useState, type MouseEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { commands, dbToFader, defaultStrip, faderToDb, formatDb, type InsertSlot, type Send } from '@ondera/core';
import { useDispatch, useSession, useStore } from '../../state/session';
import { Knob } from '../primitives/Knob';
import { LedStrip } from '../primitives/LedStrip';
import { CapsLabel } from '../primitives/CapsLabel';
import { PopupMenu, type MenuState } from '../menu/PopupMenu';
import { EFFECT_NAMES } from '../../audio/effects';
import { INSTRUMENT_NAMES } from '../../audio/instruments';
import { size } from '../../theme/tokens';
import styles from './InspectorPanel.module.css';

const PAN_DEGREES = 1.35;
const CHANNEL_SEGMENTS = 20;
const SEND_MIN_DB = -48;

const formatPan = (pan: number) => (pan === 0 ? 'C' : pan < 0 ? `L ${-Math.round(pan)}` : `R ${Math.round(pan)}`);
const sendAngle = (db: number) => (db === -Infinity ? -135 : Math.max(-135, Math.min(135, -135 + ((db - SEND_MIN_DB) / -SEND_MIN_DB) * 270)));
const faderTravel = size.faderH - size.faderCapH;
const faderCapTop = (position: number) => (1 - position) * faderTravel;

export function InspectorPanel() {
  const store = useStore();
  const dispatch = useDispatch();
  const track = useSession((s) => s.tracks.find((t) => t.id === s.view.selectedTrackId) ?? null);
  const stored = useSession((s) => (track ? s.strips[track.id] : undefined));
  const meters = useSession((s) => s.meters);
  const [menu, setMenu] = useState<MenuState | null>(null);

  if (!track) return <div className={styles.panel} />;
  const strip = stored ?? defaultStrip(track.kind);
  const db = faderToDb(track.volume);

  const onFaderDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setFromPointer(e);
  };
  const onFaderMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.currentTarget.hasPointerCapture(e.pointerId)) setFromPointer(e);
  };
  const setFromPointer = (e: ReactPointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - r.top - size.faderCapH / 2;
    const position = Math.min(1, Math.max(0, 1 - y / faderTravel));
    dispatch(commands.track.setVolume({ trackId: track.id, volume: position }));
  };

  const openInstrumentMenu = (e: MouseEvent<HTMLElement>) => {
    if (track.kind !== 'midi') return;
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      items: INSTRUMENT_NAMES.map((name) => ({
        label: name,
        checked: strip.instrument === name,
        onSelect: () => dispatch(commands.strip.setInstrument({ trackId: track.id, instrument: name })),
      })),
    });
  };

  const openInsertMenu = (slotIndex: number, slot: InsertSlot) => (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      items: [
        ...EFFECT_NAMES.map((name) => ({
          label: name,
          checked: slot.state !== 'empty' && slot.name === name,
          onSelect: () => dispatch(commands.strip.setInsert({ trackId: track.id, slotIndex, name })),
        })),
        { separator: true as const },
        {
          label: slot.state === 'bypassed' ? 'Enable' : 'Bypass',
          disabled: slot.state === 'empty',
          onSelect: () => dispatch(commands.strip.setInsertState({ trackId: track.id, slotIndex, state: slot.state === 'bypassed' ? 'active' : 'bypassed' })),
        },
        {
          label: 'Remove',
          disabled: slot.state === 'empty',
          onSelect: () => dispatch(commands.strip.setInsertState({ trackId: track.id, slotIndex, state: 'empty' })),
        },
      ],
    });
  };

  return (
    <div className={styles.panel}>
      <div className={styles.title}>
        <span className={`${styles.swatch} m-swatch`} style={{ background: track.color }} />
        <span className={styles.name}>{track.name}</span>
        <span className={styles.meta}>{track.kind === 'midi' ? 'MIDI · Ch 1' : 'AUDIO · In 1'}</span>
      </div>

      <div className={styles.rows}>
        <Row label="Instrument" value={strip.instrument} onClick={track.kind === 'midi' ? openInstrumentMenu : undefined} />
        <Row label="Input" value={strip.input} />
        <Row label="Output" value={strip.output} />
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHead}>
          <CapsLabel>Channel EQ</CapsLabel>
          <span className={styles.sectionMeta}>{strip.inserts.some((i) => i.name === 'Channel EQ' && i.state === 'active') ? 'active' : 'off · add via inserts'}</span>
        </div>
        <EqDisplay active={strip.inserts.some((i) => i.name === 'Channel EQ' && i.state === 'active')} />
      </div>

      <div className={`${styles.section} ${styles.inserts}`}>
        <CapsLabel style={{ marginBottom: 3 }}>Inserts</CapsLabel>
        {strip.inserts.map((ins, i) => (
          <Insert
            key={i}
            slot={ins}
            onClick={openInsertMenu(i, ins)}
            onToggle={() =>
              ins.state !== 'empty' &&
              dispatch(commands.strip.setInsertState({ trackId: track.id, slotIndex: i, state: ins.state === 'bypassed' ? 'active' : 'bypassed' }))
            }
          />
        ))}
      </div>

      <div className={styles.section}>
        <CapsLabel style={{ marginBottom: 8 }}>Sends</CapsLabel>
        <div className={styles.sends}>
          {strip.sends.map((s, i) => (
            <SendCard
              key={s.name}
              send={s}
              onChange={(levelDb) => dispatch(commands.strip.setSendLevel({ trackId: track.id, sendIndex: i, levelDb }))}
            />
          ))}
        </div>
      </div>

      <div className={styles.fader}>
        <div className={styles.faderLeft}>
          <CapsLabel>Pan</CapsLabel>
          <Knob
            size="lg"
            angle={track.pan * PAN_DEGREES}
            value={track.pan}
            min={-100}
            max={100}
            defaultValue={0}
            onChange={(pan) => dispatch(commands.track.setPan({ trackId: track.id, pan: Math.round(pan) }))}
          />
          <div className={styles.panValue}>{formatPan(track.pan)}</div>
          <CapsLabel style={{ marginTop: 'auto' }}>Vol</CapsLabel>
          <div className={styles.volValue} onDoubleClick={() => dispatch(commands.track.setVolume({ trackId: track.id, volume: dbToFader(0) }))} title="Double-click for 0 dB">
            {formatDb(db)}
          </div>
        </div>
        <div className={styles.faderRight}>
          <div className={`${styles.faderRail} m-groove`} onPointerDown={onFaderDown} onPointerMove={onFaderMove}>
            <div className={styles.faderCap} style={{ top: faderCapTop(track.volume) }}>
              <div className={styles.faderLine} />
            </div>
          </div>
          <div className={`${styles.channelMeter} m-well-meter`}>
            <LedStrip orientation="vertical" segments={CHANNEL_SEGMENTS} level={meters.channelL} hot={2} />
            <LedStrip orientation="vertical" segments={CHANNEL_SEGMENTS} level={meters.channelR} hot={2} />
          </div>
          <div className={styles.scale}>
            <span>+6</span>
            <span>0</span>
            <span>−6</span>
            <span>−12</span>
            <span>−24</span>
            <span>−∞</span>
          </div>
        </div>
      </div>
      {menu && <PopupMenu items={menu.items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
    </div>
  );
}

function Row({ label, value, onClick }: { label: string; value: string; onClick?: ((e: MouseEvent<HTMLElement>) => void) | undefined }) {
  return (
    <div className={`${styles.row} ${onClick ? styles.rowClickable : ''}`} onClick={onClick} title={onClick ? 'Click to change' : undefined}>
      <span className={styles.rowLabel}>{label}</span>
      <span className={styles.rowValue}>{value}</span>
    </div>
  );
}

function EqDisplay({ active }: { active: boolean }) {
  // The Channel EQ insert is a fixed low shelf +1.5, mid dip −2 at 900, high shelf +2. Flat when off.
  const path = active ? 'M0 40 C30 36 40 34 70 36 S120 46 150 44 S190 30 212 30' : 'M0 40 L212 40';
  return (
    <div className={`${styles.eq} m-well-deep`}>
      <div className={styles.eqGrid} />
      <svg width="212" height="74" viewBox="0 0 212 74" className={styles.eqSvg}>
        <defs>
          <linearGradient id="eq-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" className={styles.eqStopTop} />
            <stop offset="1" className={styles.eqStopBottom} />
          </linearGradient>
        </defs>
        <path d={`${path} L212 74 L0 74Z`} fill="url(#eq-fill)" />
        <path d={path} className={styles.eqCurve} />
        {active && <circle cx="70" cy="36" r="3" className={styles.eqHandle} />}
        {active && <circle cx="150" cy="44" r="3" className={styles.eqHandle} />}
      </svg>
    </div>
  );
}

function Insert({ slot, onClick, onToggle }: { slot: InsertSlot; onClick: (e: MouseEvent<HTMLElement>) => void; onToggle: () => void }) {
  const cls = slot.state === 'active' ? styles.insertActive : slot.state === 'bypassed' ? styles.insertBypassed : styles.insertEmpty;
  return (
    <div className={`${styles.insert} ${cls}`} onClick={onClick} title={slot.state === 'empty' ? 'Click to add an effect' : 'Click to change · LED toggles bypass'}>
      <span
        className={styles.insertLed}
        onClick={(e) => {
          e.stopPropagation();
          onToggle();
        }}
      />
      {slot.name}
      <span className={styles.insertMeta}>{slot.state === 'bypassed' ? 'bypassed' : slot.meta}</span>
    </div>
  );
}

function SendCard({ send, onChange }: { send: Send; onChange: (levelDb: number) => void }) {
  const value = send.levelDb === -Infinity ? SEND_MIN_DB : Math.max(SEND_MIN_DB, send.levelDb);
  return (
    <div className={styles.send}>
      <Knob
        size="md"
        angle={sendAngle(send.levelDb)}
        value={value}
        min={SEND_MIN_DB}
        max={0}
        defaultValue={SEND_MIN_DB}
        onChange={(v) => onChange(v <= SEND_MIN_DB + 0.5 ? -Infinity : Math.round(v * 2) / 2)}
        title="Drag to set the send level"
      />
      <div className={styles.sendText}>
        <span className={styles.sendName}>{send.name}</span>
        <span className={styles.sendValue}>{formatDb(send.levelDb)} dB</span>
      </div>
    </div>
  );
}
