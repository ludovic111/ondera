/**
 * Insert effects and send buses, by name. Each insert is a small node chain
 * with an input and an output; bypass wires input straight to output.
 */

export interface InsertNode {
  input: AudioNode;
  output: AudioNode;
}

export const EFFECT_NAMES = ['Ondera Comp', 'Channel EQ', 'Tape Sat', 'Chorus', 'Space', 'Echo'] as const;

export function isEffectName(name: string): boolean {
  return (EFFECT_NAMES as readonly string[]).includes(name);
}

export function createInsert(ctx: BaseAudioContext, name: string): InsertNode | null {
  switch (name) {
    case 'Ondera Comp': {
      const c = ctx.createDynamicsCompressor();
      c.threshold.value = -18;
      c.ratio.value = 4;
      c.attack.value = 0.005;
      c.release.value = 0.15;
      c.knee.value = 6;
      return { input: c, output: c };
    }
    case 'Channel EQ': {
      const low = ctx.createBiquadFilter();
      low.type = 'lowshelf';
      low.frequency.value = 120;
      low.gain.value = 1.5;
      const mid = ctx.createBiquadFilter();
      mid.type = 'peaking';
      mid.frequency.value = 900;
      mid.Q.value = 1;
      mid.gain.value = -2;
      const high = ctx.createBiquadFilter();
      high.type = 'highshelf';
      high.frequency.value = 6000;
      high.gain.value = 2;
      low.connect(mid);
      mid.connect(high);
      return { input: low, output: high };
    }
    case 'Tape Sat': {
      const shaper = ctx.createWaveShaper();
      const n = 1024;
      const curve = new Float32Array(n);
      for (let i = 0; i < n; i++) {
        const x = (i / (n - 1)) * 2 - 1;
        curve[i] = Math.tanh(x * 2.2) / Math.tanh(2.2);
      }
      shaper.curve = curve;
      shaper.oversample = '2x';
      const trim = ctx.createGain();
      trim.gain.value = 0.8;
      shaper.connect(trim);
      return { input: shaper, output: trim };
    }
    case 'Chorus': {
      const input = ctx.createGain();
      const output = ctx.createGain();
      const delay = ctx.createDelay(0.05);
      delay.delayTime.value = 0.018;
      const lfo = ctx.createOscillator();
      lfo.frequency.value = 0.6;
      const depth = ctx.createGain();
      depth.gain.value = 0.004;
      lfo.connect(depth);
      depth.connect(delay.delayTime);
      lfo.start();
      const wet = ctx.createGain();
      wet.gain.value = 0.5;
      input.connect(output);
      input.connect(delay);
      delay.connect(wet);
      wet.connect(output);
      return { input, output };
    }
    case 'Space': {
      const input = ctx.createGain();
      const output = ctx.createGain();
      const conv = ctx.createConvolver();
      conv.buffer = impulse(ctx, 1.8, 2.5);
      const wet = ctx.createGain();
      wet.gain.value = 0.35;
      input.connect(output);
      input.connect(conv);
      conv.connect(wet);
      wet.connect(output);
      return { input, output };
    }
    case 'Echo': {
      const input = ctx.createGain();
      const output = ctx.createGain();
      const delay = ctx.createDelay(2);
      delay.delayTime.value = 0.375;
      const fb = ctx.createGain();
      fb.gain.value = 0.35;
      const wet = ctx.createGain();
      wet.gain.value = 0.4;
      input.connect(output);
      input.connect(delay);
      delay.connect(fb);
      fb.connect(delay);
      delay.connect(wet);
      wet.connect(output);
      return { input, output };
    }
    default:
      return null;
  }
}

/** Synthetic reverb impulse: exponentially decaying noise. */
export function impulse(ctx: BaseAudioContext, seconds: number, decay: number): AudioBuffer {
  const sr = ctx.sampleRate;
  const len = Math.floor(sr * seconds);
  const buf = ctx.createBuffer(2, len, sr);
  let s = 987654;
  for (let c = 0; c < 2; c++) {
    const d = buf.getChannelData(c);
    for (let i = 0; i < len; i++) {
      s = (s * 1664525 + 1013904223) >>> 0;
      const white = (s / 4294967296) * 2 - 1;
      d[i] = white * Math.pow(1 - i / len, decay);
    }
  }
  return buf;
}

export interface SendBus {
  input: GainNode;
}

/** Bus A is a reverb, bus B a tempo-free dotted delay. Both return to the given destination. */
export function createSendBuses(ctx: BaseAudioContext, destination: AudioNode): SendBus[] {
  const a = ctx.createGain();
  const conv = ctx.createConvolver();
  conv.buffer = impulse(ctx, 2.6, 3);
  a.connect(conv);
  conv.connect(destination);

  const b = ctx.createGain();
  const delay = ctx.createDelay(2);
  delay.delayTime.value = 0.5;
  const fb = ctx.createGain();
  fb.gain.value = 0.4;
  const tone = ctx.createBiquadFilter();
  tone.type = 'lowpass';
  tone.frequency.value = 3500;
  b.connect(delay);
  delay.connect(tone);
  tone.connect(fb);
  fb.connect(delay);
  tone.connect(destination);
  return [{ input: a }, { input: b }];
}
