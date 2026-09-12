import type { ChannelStrip, InsertSlot, Send } from '@ondera/core';
import { useSession } from '../../state/session';
import { Knob } from '../primitives/Knob';
import { LedStrip } from '../primitives/LedStrip';
import { CapsLabel } from '../primitives/CapsLabel';
import { size } from '../../theme/tokens';
import styles from './InspectorPanel.module.css';

const PAN_DEGREES = 1.35;
const CHANNEL_SEGMENTS = 20;
const FADER_TOP_DB = 6;
const FADER_BOTTOM_DB = -24;
const SEND_MIN_DB = -48;

const DEFAULT_STRIP: ChannelStrip = {
  instrument: '—',
  input: 'Input 1',
  output: 'Stereo Out',
  inserts: [
    { name: 'Empty slot', meta: '', state: 'empty' },
    { name: 'Empty slot', meta: '', state: 'empty' },
    { name: 'Empty slot', meta: '', state: 'empty' },
    { name: 'Empty slot', meta: '', state: 'empty' },
  ],
  sends: [
    { name: 'A · Reverb', levelDb: -Infinity },
    { name: 'B · Delay', levelDb: -Infinity },
  ],
  volumeDb: 0,
};

const formatDb = (db: number) => (db === -Infinity ? '−∞ dB' : `${db < 0 ? '−' : ''}${Math.abs(db).toFixed(1)} dB`);
const formatPan = (pan: number) => (pan === 0 ? 'C' : pan < 0 ? `L ${-pan}` : `R ${pan}`);
const sendAngle = (db: number) =>
  db === -Infinity ? -135 : Math.max(-135, Math.min(135, -135 + ((db - SEND_MIN_DB) / -SEND_MIN_DB) * 270));
const faderCapTop = (db: number) =>
  ((FADER_TOP_DB - Math.max(FADER_BOTTOM_DB, db)) / (FADER_TOP_DB - FADER_BOTTOM_DB)) * (size.faderH - size.faderCapH);

export function InspectorPanel() {
  const track = useSession((s) => s.tracks.find((t) => t.id === s.view.selectedTrackId) ?? null);
  const strip = useSession((s) => (track ? s.strips[track.id] : undefined)) ?? DEFAULT_STRIP;
  const meters = useSession((s) => s.meters);

  if (!track) return <div className={styles.panel} />;

  return (
    <div className={styles.panel}>
      <div className={styles.title}>
        <span className={`${styles.swatch} m-swatch`} style={{ background: track.color }} />
        <span className={styles.name}>{track.name}</span>
        <span className={styles.meta}>{track.kind === 'midi' ? 'MIDI · Ch 2' : 'AUDIO · In 1'}</span>
      </div>

      <div className={styles.rows}>
        <Row label="Instrument" value={strip.instrument} />
        <Row label="Input" value={strip.input} />
        <Row label="Output" value={strip.output} />
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHead}>
          <CapsLabel>Channel EQ</CapsLabel>
          <span className={styles.sectionMeta}>4 bands</span>
        </div>
        <EqDisplay />
      </div>

      <div className={`${styles.section} ${styles.inserts}`}>
        <CapsLabel style={{ marginBottom: 3 }}>Inserts</CapsLabel>
        {strip.inserts.map((ins, i) => (
          <Insert key={i} slot={ins} />
        ))}
      </div>

      <div className={styles.section}>
        <CapsLabel style={{ marginBottom: 8 }}>Sends</CapsLabel>
        <div className={styles.sends}>
          {strip.sends.map((s) => (
            <SendCard key={s.name} send={s} />
          ))}
        </div>
      </div>

      <div className={styles.fader}>
        <div className={styles.faderLeft}>
          <CapsLabel>Pan</CapsLabel>
          <Knob size="lg" angle={track.pan * PAN_DEGREES} />
          <div className={styles.panValue}>{formatPan(track.pan)}</div>
          <CapsLabel style={{ marginTop: 'auto' }}>Vol</CapsLabel>
          <div className={styles.volValue}>{strip.volumeDb < 0 ? '−' : ''}{Math.abs(strip.volumeDb).toFixed(1)}</div>
        </div>
        <div className={styles.faderRight}>
          <div className={`${styles.faderRail} m-groove`}>
            <div className={styles.faderCap} style={{ top: faderCapTop(strip.volumeDb) }}>
              <div className={styles.faderLine} />
            </div>
          </div>
          <div className={`${styles.channelMeter} m-well-meter`}>
            <LedStrip orientation="vertical" segments={CHANNEL_SEGMENTS} level={meters.channelL} />
            <LedStrip orientation="vertical" segments={CHANNEL_SEGMENTS} level={meters.channelR} />
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
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className={styles.row}>
      <span className={styles.rowLabel}>{label}</span>
      <span className={styles.rowValue}>{value}</span>
    </div>
  );
}

function EqDisplay() {
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
        <path d="M0 52 C30 52 40 22 70 26 S120 46 150 40 S190 28 212 30 L212 74 L0 74Z" fill="url(#eq-fill)" />
        <path d="M0 52 C30 52 40 22 70 26 S120 46 150 40 S190 28 212 30" className={styles.eqCurve} />
        <circle cx="70" cy="26" r="3" className={styles.eqHandle} />
        <circle cx="150" cy="40" r="3" className={styles.eqHandle} />
      </svg>
    </div>
  );
}

function Insert({ slot }: { slot: InsertSlot }) {
  const cls = slot.state === 'active' ? styles.insertActive : slot.state === 'bypassed' ? styles.insertBypassed : styles.insertEmpty;
  return (
    <div className={`${styles.insert} ${cls}`}>
      <span className={styles.insertLed} />
      {slot.name}
      <span className={styles.insertMeta}>{slot.meta}</span>
    </div>
  );
}

function SendCard({ send }: { send: Send }) {
  return (
    <div className={styles.send}>
      <Knob size="md" angle={sendAngle(send.levelDb)} />
      <div className={styles.sendText}>
        <span className={styles.sendName}>{send.name}</span>
        <span className={styles.sendValue}>{formatDb(send.levelDb)}</span>
      </div>
    </div>
  );
}
