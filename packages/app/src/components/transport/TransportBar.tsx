import { useState, type MouseEvent } from 'react';
import { commands, formatDb } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
import { PopupMenu, type MenuState } from '../menu/PopupMenu';
import { SNAP_DIVISIONS } from '../../state/actions';
import { Button } from '../primitives/Button';
import { LedStrip } from '../primitives/LedStrip';
import { CapsLabel } from '../primitives/CapsLabel';
import { CycleIcon, ForwardIcon, PlayIcon, RecordIcon, ReturnIcon, RewindIcon, StopIcon } from '../primitives/Icons';
import { TimeDisplay } from './TimeDisplay';
import { size } from '../../theme/tokens';
import styles from './TransportBar.module.css';

const MASTER_SEGMENTS = 22;
const CPU_SEGMENTS = 12;

export function TransportBar() {
  const dispatch = useDispatch();
  const playing = useSession((s) => s.transport.playing);
  const recording = useSession((s) => s.transport.recording);
  const cycle = useSession((s) => s.transport.cycle);
  const metronome = useSession((s) => s.transport.metronome);
  const snap = useSession((s) => s.transport.snapDivision);
  const agentOpen = useSession((s) => s.view.agentPanelOpen);
  const meters = useSession((s) => s.meters);
  const [menu, setMenu] = useState<MenuState | null>(null);

  const openSnapMenu = (e: MouseEvent<HTMLElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setMenu({
      x: r.left,
      y: r.bottom + 4,
      items: SNAP_DIVISIONS.map((d) => ({
        label: d === 1 ? 'Bar' : `1/${d}`,
        checked: snap === d,
        onSelect: () => dispatch(commands.transport.setSnap({ division: d })),
      })),
    });
  };
  // Master readout: the loudest side, in dB, from the engine meters.
  const masterDb = 20 * Math.log10(Math.max(1e-4, Math.pow(10, (Math.max(meters.masterL, meters.masterR) * 60 - 54) / 20)));

  return (
    <div className={styles.bar}>
      <div className={styles.group}>
        <Button title="Return" onClick={() => dispatch(commands.transport.returnToStart({}))}>
          <ReturnIcon />
        </Button>
        <Button title="Rewind" onClick={() => dispatch(commands.transport.nudge({ bars: -1 }))}>
          <RewindIcon />
        </Button>
        <Button title="Forward" onClick={() => dispatch(commands.transport.nudge({ bars: 1 }))}>
          <ForwardIcon />
        </Button>
      </div>
      <div className={styles.group}>
        <Button title="Play (Space)" size="wide" pressed={playing} onClick={() => dispatch(commands.transport.play({}))}>
          <PlayIcon />
        </Button>
        <Button title="Stop · twice to return to start" onClick={() => dispatch(commands.transport.stop({}))}>
          <StopIcon />
        </Button>
        <Button
          title="Record (R) · arm an audio track, then play"
          lit={recording}
          onClick={() => dispatch(commands.transport.setRecording({ recording: !recording }))}
        >
          <RecordIcon />
        </Button>
        <Button title="Cycle (C) · drag in the ruler to set the range" pressed={cycle} onClick={() => dispatch(commands.transport.setCycle({ enabled: !cycle }))}>
          <CycleIcon />
        </Button>
      </div>

      <TimeDisplay />

      <div className={styles.group}>
        <Button
          size="auto"
          title="Metronome (K)"
          pressed={metronome}
          onClick={() => dispatch(commands.transport.setMetronome({ enabled: !metronome }))}
        >
          Click
        </Button>
        <Button size="auto" pressed onClick={openSnapMenu} title="Snap grid">
          Snap {snap === 1 ? 'Bar' : `1/${snap}`}
        </Button>
      </div>

      <div className={styles.right}>
        <div className={styles.meter}>
          <CapsLabel>Master</CapsLabel>
          <div className={`${styles.masterWell} m-well-meter`}>
            <LedStrip segments={MASTER_SEGMENTS} level={meters.masterL} hot={2} />
            <LedStrip segments={MASTER_SEGMENTS} level={meters.masterR} hot={2} />
          </div>
          <div className={styles.readout}>{Math.max(meters.masterL, meters.masterR) > 0 ? formatDb(masterDb, 1) : '−∞'}</div>
        </div>
        <div className={styles.cpu}>
          <CapsLabel>CPU</CapsLabel>
          <div className={`${styles.cpuWell} m-well-meter`}>
            <LedStrip segments={CPU_SEGMENTS} level={meters.cpu} segmentHeight={size.ledCpuH} />
          </div>
          <div className={styles.readout}>{Math.round(meters.cpu * 100)}%</div>
        </div>
        <Button
          size="auto"
          pressed={agentOpen}
          className={styles.agentButton}
          onClick={() => dispatch(commands.view.setAgentPanelOpen({ open: !agentOpen }))}
        >
          <span className={`${styles.agentDot} m-accent-dot`} />
          Agent
        </Button>
      </div>
      {menu && <PopupMenu items={menu.items} x={menu.x} y={menu.y} onClose={() => setMenu(null)} />}
    </div>
  );
}
