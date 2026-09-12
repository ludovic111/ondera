/**
 * Small polyphonic synths, one per MIDI track, picked by preset name (the
 * strip's `instrument`). Each note is a fire-and-forget node graph with
 * scheduled envelopes, so scheduling ahead of time is cheap and offline
 * rendering works unchanged.
 */

export interface Preset {
  name: string;
  kind: 'subtractive' | 'fm' | 'pluck' | 'pad' | 'drums' | 'sub' | 'bell' | 'riser';
  gain: number;
}

const PRESETS: Record<string, Preset> = {
  'Ondera Synth': { name: 'Ondera Synth', kind: 'subtractive', gain: 0.32 },
  'E-Piano Mk I': { name: 'E-Piano Mk I', kind: 'fm', gain: 0.5 },
  'Drum Machine': { name: 'Drum Machine', kind: 'drums', gain: 0.9 },
  Sampler: { name: 'Sampler', kind: 'pluck', gain: 0.5 },
  'Sub Bass 808': { name: 'Sub Bass 808', kind: 'sub', gain: 0.7 },
  'Glass Keys': { name: 'Glass Keys', kind: 'bell', gain: 0.45 },
  'Choir Pad': { name: 'Choir Pad', kind: 'pad', gain: 0.28 },
  Riser: { name: 'Riser', kind: 'riser', gain: 0.4 },
};

export const INSTRUMENT_NAMES = Object.keys(PRESETS);

export function presetFor(name: string): Preset {
  return PRESETS[name] ?? PRESETS['Ondera Synth']!;
}

const midiToHz = (pitch: number) => 440 * Math.pow(2, (pitch - 69) / 12);

interface Voice {
  stop(at: number): void;
}

export class Instrument {
  readonly output: GainNode;
  private preset: Preset;
  private voices = new Set<Voice>();
  private noiseBuffer: AudioBuffer | null = null;

  constructor(
    private readonly ctx: BaseAudioContext,
    presetName: string,
  ) {
    this.preset = presetFor(presetName);
    this.output = ctx.createGain();
    this.output.gain.value = this.preset.gain;
  }

  get presetName(): string {
    return this.preset.name;
  }

  setPreset(name: string): void {
    this.preset = presetFor(name);
    this.output.gain.setTargetAtTime(this.preset.gain, this.ctx.currentTime, 0.02);
  }

  /** Schedule one note. `end` is when the key is released; the tail follows. */
  play(pitch: number, velocity: number, start: number, end: number): void {
    const vel = Math.max(0.05, Math.min(1, velocity / 127));
    const dur = Math.max(0.03, end - start);
    switch (this.preset.kind) {
      case 'subtractive':
        this.subtractive(pitch, vel, start, dur, 'sawtooth', 0.01, 0.25, 0.6, 0.18, 2200);
        break;
      case 'pad':
        this.subtractive(pitch, vel, start, dur, 'sawtooth', 0.35, 0.8, 0.8, 0.6, 1200, 7);
        break;
      case 'sub':
        this.subtractive(pitch, vel, start, dur, 'sine', 0.005, 0.3, 0.9, 0.2, 400);
        break;
      case 'fm':
        this.fm(pitch, vel, start, dur, 1, 1.8, 1.6);
        break;
      case 'bell':
        this.fm(pitch, vel, start, dur, 3.5, 2.5, 3.2);
        break;
      case 'pluck':
        this.subtractive(pitch, vel, start, Math.min(dur, 0.4), 'triangle', 0.002, 0.18, 0.0, 0.12, 3000);
        break;
      case 'drums':
        this.drum(pitch, vel, start);
        break;
      case 'riser':
        this.riser(vel, start, dur);
        break;
    }
  }

  /** Hard-stop everything (transport stop, locate). */
  panic(at = this.ctx.currentTime): void {
    for (const v of this.voices) v.stop(at);
    this.voices.clear();
  }

  private track(v: Voice, until: number): void {
    this.voices.add(v);
    if (typeof setTimeout === 'function' && 'currentTime' in this.ctx) {
      const ms = Math.max(0, (until - this.ctx.currentTime) * 1000 + 50);
      setTimeout(() => this.voices.delete(v), ms);
    }
  }

  private subtractive(
    pitch: number,
    vel: number,
    start: number,
    dur: number,
    type: OscillatorType,
    a: number,
    d: number,
    s: number,
    r: number,
    cutoff: number,
    detuneCents = 0,
  ): void {
    const ctx = this.ctx;
    const hz = midiToHz(pitch);
    const oscs: OscillatorNode[] = [];
    const filter = ctx.createBiquadFilter();
    filter.type = 'lowpass';
    filter.Q.value = 0.9;
    filter.frequency.setValueAtTime(cutoff * (0.5 + vel), start);
    filter.frequency.exponentialRampToValueAtTime(Math.max(200, cutoff * 0.35), start + a + d);
    const amp = ctx.createGain();
    amp.gain.setValueAtTime(0, start);
    amp.gain.linearRampToValueAtTime(vel, start + a);
    amp.gain.setTargetAtTime(vel * s, start + a, d / 3);
    const release = start + dur;
    amp.gain.setValueAtTime(vel * s + (amp.gain.value - vel * s) * 0, release);
    amp.gain.setTargetAtTime(0, release, r / 4);
    const stopAt = release + r * 1.5;
    const detunes = detuneCents ? [-detuneCents, detuneCents] : [0];
    for (const dt of detunes) {
      const osc = ctx.createOscillator();
      osc.type = type;
      osc.frequency.value = hz;
      osc.detune.value = dt;
      osc.connect(filter);
      osc.start(start);
      osc.stop(stopAt);
      oscs.push(osc);
    }
    filter.connect(amp);
    amp.connect(this.output);
    this.track(
      {
        stop: (at) => {
          amp.gain.cancelScheduledValues(at);
          amp.gain.setTargetAtTime(0, at, 0.01);
          for (const o of oscs) o.stop(at + 0.05);
        },
      },
      stopAt,
    );
  }

  private fm(pitch: number, vel: number, start: number, dur: number, ratio: number, index: number, decay: number): void {
    const ctx = this.ctx;
    const hz = midiToHz(pitch);
    const carrier = ctx.createOscillator();
    carrier.type = 'sine';
    carrier.frequency.value = hz;
    const mod = ctx.createOscillator();
    mod.type = 'sine';
    mod.frequency.value = hz * ratio;
    const modGain = ctx.createGain();
    modGain.gain.setValueAtTime(hz * index * vel, start);
    modGain.gain.exponentialRampToValueAtTime(hz * 0.05, start + decay);
    mod.connect(modGain);
    modGain.connect(carrier.frequency);
    const amp = ctx.createGain();
    amp.gain.setValueAtTime(0, start);
    amp.gain.linearRampToValueAtTime(vel, start + 0.005);
    amp.gain.setTargetAtTime(vel * 0.25, start + 0.005, decay / 3);
    const release = start + dur;
    amp.gain.setTargetAtTime(0, release, 0.08);
    const stopAt = release + 0.5;
    carrier.connect(amp);
    amp.connect(this.output);
    carrier.start(start);
    mod.start(start);
    carrier.stop(stopAt);
    mod.stop(stopAt);
    this.track(
      {
        stop: (at) => {
          amp.gain.cancelScheduledValues(at);
          amp.gain.setTargetAtTime(0, at, 0.01);
          carrier.stop(at + 0.05);
          mod.stop(at + 0.05);
        },
      },
      stopAt,
    );
  }

  private noise(): AudioBuffer {
    if (this.noiseBuffer) return this.noiseBuffer;
    const sr = this.ctx.sampleRate;
    const buf = this.ctx.createBuffer(1, sr, sr);
    const d = buf.getChannelData(0);
    let s = 12345;
    for (let i = 0; i < d.length; i++) {
      s = (s * 1664525 + 1013904223) >>> 0;
      d[i] = (s / 4294967296) * 2 - 1;
    }
    this.noiseBuffer = buf;
    return buf;
  }

  /** GM-ish mapping: kick 35/36, snare 38/40, clap 39, closed hat 42/44, open hat 46, toms 41–50, crash 49. */
  private drum(pitch: number, vel: number, start: number): void {
    const ctx = this.ctx;
    const cls = pitch % 12;
    const isKick = pitch === 35 || pitch === 36 || cls === 0;
    const isSnare = pitch === 38 || pitch === 40 || cls === 2 || cls === 4;
    const isClap = pitch === 39 || cls === 3;
    const isOpenHat = pitch === 46 || cls === 10;
    const isCrash = pitch === 49 || cls === 1;
    if (isKick) {
      const osc = ctx.createOscillator();
      osc.frequency.setValueAtTime(160, start);
      osc.frequency.exponentialRampToValueAtTime(45, start + 0.12);
      const amp = ctx.createGain();
      amp.gain.setValueAtTime(vel, start);
      amp.gain.exponentialRampToValueAtTime(0.001, start + 0.4);
      osc.connect(amp);
      amp.connect(this.output);
      osc.start(start);
      osc.stop(start + 0.45);
      return;
    }
    const src = ctx.createBufferSource();
    src.buffer = this.noise();
    const filter = ctx.createBiquadFilter();
    const amp = ctx.createGain();
    let len = 0.15;
    if (isSnare || isClap) {
      filter.type = 'bandpass';
      filter.frequency.value = isClap ? 1500 : 1800;
      filter.Q.value = 0.8;
      len = 0.2;
      const tone = ctx.createOscillator();
      tone.frequency.setValueAtTime(isClap ? 300 : 200, start);
      tone.frequency.exponentialRampToValueAtTime(120, start + 0.1);
      const tAmp = ctx.createGain();
      tAmp.gain.setValueAtTime(vel * 0.5, start);
      tAmp.gain.exponentialRampToValueAtTime(0.001, start + 0.12);
      tone.connect(tAmp);
      tAmp.connect(this.output);
      tone.start(start);
      tone.stop(start + 0.15);
    } else if (isCrash) {
      filter.type = 'highpass';
      filter.frequency.value = 5000;
      len = 1.2;
    } else if (isOpenHat) {
      filter.type = 'highpass';
      filter.frequency.value = 7000;
      len = 0.4;
    } else {
      filter.type = 'highpass';
      filter.frequency.value = 8000;
      len = 0.07;
    }
    amp.gain.setValueAtTime(vel * 0.8, start);
    amp.gain.exponentialRampToValueAtTime(0.001, start + len);
    src.connect(filter);
    filter.connect(amp);
    amp.connect(this.output);
    src.start(start);
    src.stop(start + len + 0.05);
  }

  private riser(vel: number, start: number, dur: number): void {
    const ctx = this.ctx;
    const src = ctx.createBufferSource();
    src.buffer = this.noise();
    src.loop = true;
    const filter = ctx.createBiquadFilter();
    filter.type = 'bandpass';
    filter.Q.value = 2;
    filter.frequency.setValueAtTime(300, start);
    filter.frequency.exponentialRampToValueAtTime(6000, start + dur);
    const amp = ctx.createGain();
    amp.gain.setValueAtTime(0.001, start);
    amp.gain.exponentialRampToValueAtTime(vel, start + dur);
    amp.gain.setTargetAtTime(0, start + dur, 0.05);
    src.connect(filter);
    filter.connect(amp);
    amp.connect(this.output);
    src.start(start);
    src.stop(start + dur + 0.3);
    this.track({ stop: (at) => src.stop(at + 0.02) }, start + dur + 0.3);
  }
}
