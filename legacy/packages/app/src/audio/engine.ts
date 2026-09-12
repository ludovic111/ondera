/**
 * Realtime Web Audio engine behind core's EngineClient. Stands in for the
 * Rust process: same interface, same store wiring, so swapping later is a
 * one-line change in main.tsx.
 */
import { barsToBeats, beatsToBars, type EngineClient, type Meters, type Session } from '@ondera/core';
import { Graph } from './graph';
import { library } from './library';

const LOOKAHEAD_S = 0.12;
const SCHEDULE_MS = 25;
const METER_MS = 40;

const peakToLevel = (peak: number) => {
  if (peak <= 0) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 54) / 60));
};

export class WebAudioEngine implements EngineClient {
  readonly ctx: AudioContext;
  private graph: Graph;
  private state: Session | null = null;
  private playing = false;
  private tempo = 120;
  /** Beats at `anchorTime`; the playhead is anchorBeats + elapsed × tempo. */
  private anchorBeats = 0;
  private anchorTime = 0;
  private scheduledUntil = 0;
  private startMidClips = true;
  private timer: ReturnType<typeof setInterval> | null = null;
  private meterTimer: ReturnType<typeof setInterval> | null = null;
  private tickListeners = new Set<(beats: number) => void>();
  private meterListeners = new Set<(m: Meters) => void>();
  private scratch = new Float32Array(512);
  private selectedTrackId: string | null = null;

  constructor() {
    this.ctx = new AudioContext({ sampleRate: 48000, latencyHint: 'interactive' });
    library.attach(this.ctx);
    this.graph = new Graph(this.ctx);
    this.meterTimer = setInterval(this.meter, METER_MS);
  }

  /** Browsers gate audio behind a gesture; call from a pointer/key handler. */
  resume(): void {
    if (this.ctx.state !== 'running') void this.ctx.resume();
  }

  sync(state: Session): void {
    this.state = state;
    this.selectedTrackId = state.view.selectedTrackId;
    this.graph.sync(state);
    if (state.transport.tempo !== this.tempo) this.setTempo(state.transport.tempo);
  }

  start(fromBeats: number, tempo: number): void {
    this.resume();
    this.tempo = tempo;
    this.playing = true;
    this.anchorBeats = fromBeats;
    this.anchorTime = this.ctx.currentTime + 0.03;
    this.scheduledUntil = fromBeats;
    this.startMidClips = true;
    if (!this.timer) this.timer = setInterval(this.pump, SCHEDULE_MS);
    this.pump();
  }

  stop(): void {
    this.playing = false;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    this.graph.panic(this.ctx.currentTime);
  }

  locate(beats: number): void {
    if (!this.playing) {
      this.anchorBeats = beats;
      return;
    }
    // Jump while playing: silence everything and restart the scheduler from the new spot.
    this.graph.panic(this.ctx.currentTime);
    this.anchorBeats = beats;
    this.anchorTime = this.ctx.currentTime + 0.02;
    this.scheduledUntil = beats;
    this.startMidClips = true;
    this.pump();
  }

  setTempo(bpm: number): void {
    if (this.playing) {
      const pos = this.position();
      this.graph.panic(this.ctx.currentTime);
      this.anchorBeats = pos;
      this.anchorTime = this.ctx.currentTime + 0.02;
      this.scheduledUntil = pos;
      this.startMidClips = true;
    }
    this.tempo = bpm;
  }

  onTick(cb: (beats: number) => void): () => void {
    this.tickListeners.add(cb);
    return () => {
      this.tickListeners.delete(cb);
    };
  }

  onMeters(cb: (m: Meters) => void): () => void {
    this.meterListeners.add(cb);
    return () => {
      this.meterListeners.delete(cb);
    };
  }

  previewNote(trackId: string, pitch: number, velocity: number): void {
    this.resume();
    this.graph.previewNote(trackId, pitch, velocity);
  }

  dispose(): void {
    this.stop();
    if (this.meterTimer) clearInterval(this.meterTimer);
    this.graph.dispose();
    void this.ctx.close();
  }

  private position(): number {
    return this.anchorBeats + Math.max(0, this.ctx.currentTime - this.anchorTime) * (this.tempo / 60);
  }

  private cycle(): { start: number; end: number } | null {
    const t = this.state?.transport;
    if (!t || !t.cycle) return null;
    const start = barsToBeats(t.cycleStartBar, t.timeSignature);
    const end = barsToBeats(t.cycleEndBar, t.timeSignature);
    return end > start ? { start, end } : null;
  }

  /** Scheduler heartbeat: schedule events up to the lookahead horizon, wrapping at the cycle end. */
  private pump = (): void => {
    if (!this.playing || !this.state) return;
    const secPerBeat = 60 / this.tempo;
    const horizonTime = this.ctx.currentTime + LOOKAHEAD_S;
    for (let guard = 0; guard < 8; guard++) {
      const horizonBeats = this.anchorBeats + (horizonTime - this.anchorTime) / secPerBeat;
      if (horizonBeats <= this.scheduledUntil) break;
      const cyc = this.cycle();
      const inCycle = cyc !== null && this.scheduledUntil < cyc.end && this.scheduledUntil >= cyc.start - 1e-9;
      const segEnd = inCycle ? Math.min(horizonBeats, cyc.end) : horizonBeats;
      this.graph.scheduleRange({
        fromBeats: this.scheduledUntil,
        toBeats: segEnd,
        atTime: this.anchorTime + (this.scheduledUntil - this.anchorBeats) * secPerBeat,
        hardEndBeats: inCycle ? cyc.end : Number.POSITIVE_INFINITY,
        startMidClips: this.startMidClips,
      });
      this.startMidClips = false;
      this.scheduledUntil = segEnd;
      if (inCycle && segEnd >= cyc.end - 1e-9) {
        // Wrap: the cycle start sounds exactly when the cycle end would have.
        this.anchorTime = this.anchorTime + (cyc.end - this.anchorBeats) * secPerBeat;
        this.anchorBeats = cyc.start;
        this.scheduledUntil = cyc.start;
        this.startMidClips = true;
      }
    }
    const pos = this.position();
    for (const l of this.tickListeners) l(pos);
  };

  private peakOf(analyser: AnalyserNode): number {
    analyser.getFloatTimeDomainData(this.scratch);
    let peak = 0;
    for (let i = 0; i < this.scratch.length; i++) {
      const v = Math.abs(this.scratch[i]!);
      if (v > peak) peak = v;
    }
    return peak;
  }

  private lastMeters: Meters = { masterL: 0, masterR: 0, cpu: 0, channelL: 0, channelR: 0 };

  private meter = (): void => {
    if (this.meterListeners.size === 0 || this.ctx.state !== 'running') return;
    const masterL = peakToLevel(this.peakOf(this.graph.masterAnalyserL));
    const masterR = peakToLevel(this.peakOf(this.graph.masterAnalyserR));
    const ch = this.selectedTrackId ? this.graph.channel(this.selectedTrackId) : undefined;
    const chPeak = ch ? peakToLevel(this.peakOf(ch.analyser)) : 0;
    const cpu = Math.min(1, (this.ctx.baseLatency + 0.002) * 4 + (this.playing ? 0.12 : 0.03));
    const next: Meters = { masterL, masterR, cpu, channelL: chPeak, channelR: chPeak };
    const same = (a: number, b: number) => Math.abs(a - b) < 0.01;
    if (
      same(next.masterL, this.lastMeters.masterL) &&
      same(next.masterR, this.lastMeters.masterR) &&
      same(next.channelL, this.lastMeters.channelL) &&
      same(next.cpu, this.lastMeters.cpu)
    ) {
      return;
    }
    this.lastMeters = next;
    for (const l of this.meterListeners) l(next);
  };

  /** Bar position of the playhead, for callers that think in bars. */
  positionBars(): number {
    const sig = this.state?.transport.timeSignature ?? { numerator: 4, denominator: 4 };
    return beatsToBars(this.position(), sig);
  }
}
