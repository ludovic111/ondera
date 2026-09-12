import { commands } from '@ondera/core';
import { useDispatch, useSession } from '../../state/session';
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
        <Button title="Play" size="wide" pressed={playing} onClick={() => dispatch(commands.transport.play({}))}>
          <PlayIcon />
        </Button>
        <Button title="Stop" onClick={() => dispatch(commands.transport.stop({}))}>
          <StopIcon />
        </Button>
        <Button
          title="Record"
          lit={recording}
          onClick={() => dispatch(commands.transport.setRecording({ recording: !recording }))}
        >
          <RecordIcon />
        </Button>
        <Button title="Cycle" pressed={cycle} onClick={() => dispatch(commands.transport.setCycle({ enabled: !cycle }))}>
          <CycleIcon />
        </Button>
      </div>

      <TimeDisplay />

      <div className={styles.group}>
        <Button
          size="auto"
          pressed={metronome}
          onClick={() => dispatch(commands.transport.setMetronome({ enabled: !metronome }))}
        >
          Click
        </Button>
        <Button size="auto" pressed>
          Snap 1/{snap}
        </Button>
      </div>

      <div className={styles.right}>
        <div className={styles.meter}>
          <CapsLabel>Master</CapsLabel>
          <div className={`${styles.masterWell} m-well-meter`}>
            <LedStrip segments={MASTER_SEGMENTS} level={meters.masterL} />
            <LedStrip segments={MASTER_SEGMENTS} level={meters.masterR} />
          </div>
          <div className={styles.readout}>−3.2</div>
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
    </div>
  );
}
