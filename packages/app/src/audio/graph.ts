/**
 * The mixer graph and the event scheduler, independent of realtime vs
 * offline. One Channel per track: source (instrument or clip players) →
 * inserts → fader → pan → mute → master, plus sends. `scheduleRange`
 * schedules every note, clip and click whose start falls in a beat range.
 */
import {
  barsToBeats,
  beatsPerBar,
  beatsToSeconds,
  faderToDb,
  type ChannelStrip,
  type Session,
  type Track,
  defaultStrip,
} from '@ondera/core';
import { Instrument } from './instruments';
import { createInsert, createSendBuses, type InsertNode, type SendBus } from './effects';
import { library } from './library';

const dbToGain = (db: number) => (db === -Infinity ? 0 : Math.pow(10, db / 20));

class Channel {
  readonly input: GainNode;
  readonly fader: GainNode;
  readonly pan: StereoPannerNode;
  readonly mute: GainNode;
  readonly analyser: AnalyserNode;
  readonly sendGains: GainNode[] = [];
  instrument: Instrument | null = null;
  private inserts: { name: string; state: string; node: InsertNode | null }[] = [];
  private insertsIn: GainNode;
  private players = new Set<AudioBufferSourceNode>();

  constructor(
    private readonly ctx: BaseAudioContext,
    track: Track,
    strip: ChannelStrip,
    master: AudioNode,
    buses: SendBus[],
  ) {
    this.input = ctx.createGain();
    this.insertsIn = ctx.createGain();
    this.fader = ctx.createGain();
    this.pan = ctx.createStereoPanner();
    this.mute = ctx.createGain();
    this.analyser = ctx.createAnalyser();
    this.analyser.fftSize = 512;
    this.input.connect(this.insertsIn);
    this.fader.connect(this.pan);
    this.pan.connect(this.mute);
    this.mute.connect(master);
    this.mute.connect(this.analyser);
    for (const bus of buses) {
      const g = ctx.createGain();
      g.gain.value = 0;
      this.fader.connect(g);
      g.connect(bus.input);
      this.sendGains.push(g);
    }
    if (track.kind === 'midi') {
      this.instrument = new Instrument(ctx, strip.instrument);
      this.instrument.output.connect(this.input);
    }
    this.rebuildInserts(strip);
  }

  /** Recreate the insert chain when slot names or states changed. */
  rebuildInserts(strip: ChannelStrip): void {
    const same =
      this.inserts.length === strip.inserts.length &&
      this.inserts.every((s, i) => s.name === strip.inserts[i]!.name && s.state === strip.inserts[i]!.state);
    if (same && this.inserts.length) return;
    this.insertsIn.disconnect();
    for (const s of this.inserts) s.node?.output.disconnect();
    this.inserts = strip.inserts.map((slot) => ({
      name: slot.name,
      state: slot.state,
      node: slot.state === 'active' ? createInsert(this.ctx, slot.name) : null,
    }));
    let head: AudioNode = this.insertsIn;
    for (const s of this.inserts) {
      if (!s.node) continue;
      head.connect(s.node.input);
      head = s.node.output;
    }
    head.connect(this.fader);
  }

  apply(track: Track, strip: ChannelStrip, effectiveMute: boolean, now: number): void {
    const tc = 0.015;
    this.fader.gain.setTargetAtTime(dbToGain(faderToDb(track.volume)), now, tc);
    this.pan.pan.setTargetAtTime(track.pan / 100, now, tc);
    this.mute.gain.setTargetAtTime(effectiveMute ? 0 : 1, now, tc);
    strip.sends.forEach((s, i) => this.sendGains[i]?.gain.setTargetAtTime(dbToGain(s.levelDb), now, tc));
    if (this.instrument && this.instrument.presetName !== strip.instrument) this.instrument.setPreset(strip.instrument);
    this.rebuildInserts(strip);
  }

  playBuffer(buffer: AudioBuffer, when: number, offset: number, duration: number): void {
    if (duration <= 0 || offset >= buffer.duration) return;
    const src = this.ctx.createBufferSource();
    src.buffer = buffer;
    src.connect(this.input);
    src.start(when, offset, Math.min(duration, buffer.duration - offset));
    this.players.add(src);
    src.onended = () => this.players.delete(src);
  }

  panic(at: number): void {
    this.instrument?.panic(at);
    for (const p of this.players) {
      try {
        p.stop(at);
      } catch {
        /* already stopped */
      }
    }
    this.players.clear();
  }

  dispose(): void {
    this.panic(this.ctx.currentTime);
    this.mute.disconnect();
    this.analyser.disconnect();
    for (const g of this.sendGains) g.disconnect();
  }
}

export interface Range {
  fromBeats: number;
  toBeats: number;
  /** Context time at which `fromBeats` sounds. */
  atTime: number;
  /** Beat past which nothing is allowed to sound (cycle end); events are truncated there. */
  hardEndBeats: number;
  /** Also start clips that already began before fromBeats (after a locate or a loop wrap). */
  startMidClips: boolean;
}

export class Graph {
  readonly master: GainNode;
  readonly masterAnalyserL: AnalyserNode;
  readonly masterAnalyserR: AnalyserNode;
  private readonly splitter: ChannelSplitterNode;
  private readonly buses: SendBus[];
  private channels = new Map<string, Channel>();
  private click: GainNode;
  private state: Session | null = null;

  constructor(readonly ctx: BaseAudioContext) {
    this.master = ctx.createGain();
    this.master.gain.value = 0.9;
    const limiter = ctx.createDynamicsCompressor();
    limiter.threshold.value = -3;
    limiter.ratio.value = 12;
    limiter.attack.value = 0.002;
    limiter.release.value = 0.1;
    this.master.connect(limiter);
    limiter.connect(ctx.destination);
    this.splitter = ctx.createChannelSplitter(2);
    limiter.connect(this.splitter);
    this.masterAnalyserL = ctx.createAnalyser();
    this.masterAnalyserR = ctx.createAnalyser();
    this.masterAnalyserL.fftSize = 512;
    this.masterAnalyserR.fftSize = 512;
    this.splitter.connect(this.masterAnalyserL, 0);
    this.splitter.connect(this.masterAnalyserR, 1);
    this.buses = createSendBuses(ctx, this.master);
    this.click = ctx.createGain();
    this.click.gain.value = 0.5;
    this.click.connect(this.master);
  }

  channel(trackId: string): Channel | undefined {
    return this.channels.get(trackId);
  }

  /** Mirror the session: create/dispose channels, apply mix settings. */
  sync(state: Session): void {
    this.state = state;
    const now = this.ctx.currentTime;
    const anySolo = state.tracks.some((t) => t.solo);
    const seen = new Set<string>();
    for (const track of state.tracks) {
      seen.add(track.id);
      const strip = state.strips[track.id] ?? defaultStrip(track.kind);
      let ch = this.channels.get(track.id);
      if (!ch) {
        ch = new Channel(this.ctx, track, strip, this.master, this.buses);
        this.channels.set(track.id, ch);
      }
      ch.apply(track, strip, track.mute || (anySolo && !track.solo), now);
    }
    for (const [id, ch] of this.channels) {
      if (!seen.has(id)) {
        ch.dispose();
        this.channels.delete(id);
      }
    }
    library.ensureGenerated(state.sources);
  }

  /** Schedule every event starting in [fromBeats, toBeats). */
  scheduleRange(range: Range): void {
    const state = this.state;
    if (!state) return;
    const { tempo, timeSignature, metronome } = state.transport;
    const secPerBeat = 60 / tempo;
    const timeAt = (beat: number) => range.atTime + (beat - range.fromBeats) * secPerBeat;
    const bpb = beatsPerBar(timeSignature);

    for (const clip of state.clips) {
      const ch = this.channels.get(clip.trackId);
      if (!ch) continue;
      const clipStart = barsToBeats(clip.startBar, timeSignature);
      const clipEnd = Math.min(clipStart + barsToBeats(clip.lengthBars, timeSignature), range.hardEndBeats);
      if (clipEnd <= range.fromBeats || clipStart >= range.toBeats) continue;

      if (clip.data.kind === 'midi') {
        if (!ch.instrument) continue;
        for (const n of clip.data.notes) {
          const abs = clipStart + n.start;
          if (abs < range.fromBeats || abs >= range.toBeats || abs >= clipEnd) continue;
          const end = Math.min(abs + n.length, clipEnd);
          ch.instrument.play(n.pitch, n.velocity, timeAt(abs), timeAt(end));
        }
      } else {
        const buffer = library.get(clip.data.sourceId);
        if (!buffer) continue;
        const startsHere = clipStart >= range.fromBeats;
        if (!startsHere && !range.startMidClips) continue;
        const from = startsHere ? clipStart : range.fromBeats;
        const offset = clip.data.offsetSeconds + (startsHere ? 0 : beatsToSeconds(from - clipStart, tempo));
        ch.playBuffer(buffer, timeAt(from), offset, beatsToSeconds(clipEnd - from, tempo));
      }
    }

    if (metronome) {
      const first = Math.ceil(range.fromBeats - 1e-6);
      for (let b = first; b < range.toBeats && b < range.hardEndBeats; b++) {
        this.tick(timeAt(b), b % bpb === 0);
      }
    }
  }

  private tick(at: number, accent: boolean): void {
    const osc = this.ctx.createOscillator();
    osc.frequency.value = accent ? 1600 : 1100;
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(accent ? 0.7 : 0.4, at);
    g.gain.exponentialRampToValueAtTime(0.001, at + 0.04);
    osc.connect(g);
    g.connect(this.click);
    osc.start(at);
    osc.stop(at + 0.05);
  }

  previewNote(trackId: string, pitch: number, velocity: number): void {
    const ch = this.channels.get(trackId);
    const now = this.ctx.currentTime;
    ch?.instrument?.play(pitch, velocity, now, now + 0.3);
  }

  panic(at = this.ctx.currentTime): void {
    for (const ch of this.channels.values()) ch.panic(at);
  }

  dispose(): void {
    for (const ch of this.channels.values()) ch.dispose();
    this.channels.clear();
  }
}
