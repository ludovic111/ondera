/**
 * Owns the samples behind every AudioSource in the session. Core only knows
 * metadata; the buffers live here, keyed by source id. Also caches waveform
 * peaks for the canvas.
 */
import type { AudioSource } from '@ondera/core';
import { generateBuffer } from './generate';

/** Peak columns per second of audio in the cache. Enough for ~1 px per column at max zoom. */
export const PEAKS_PER_SECOND = 400;

type Listener = () => void;

class AudioLibrary {
  private buffers = new Map<string, AudioBuffer>();
  private peaks = new Map<string, Float32Array>();
  private pending = new Set<string>();
  private listeners = new Set<Listener>();
  private ctx: BaseAudioContext | null = null;

  attach(ctx: BaseAudioContext): void {
    this.ctx = ctx;
  }

  onChange(cb: Listener): () => void {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  }

  private notify(): void {
    for (const l of this.listeners) l();
  }

  has(id: string): boolean {
    return this.buffers.has(id);
  }

  get(id: string): AudioBuffer | undefined {
    return this.buffers.get(id);
  }

  put(id: string, buffer: AudioBuffer): void {
    this.buffers.set(id, buffer);
    this.peaks.delete(id);
    this.notify();
  }

  /** Make sure every generated source in the session has a buffer. Files must be put() by the host. */
  ensureGenerated(sources: Record<string, AudioSource>): void {
    if (!this.ctx) return;
    for (const src of Object.values(sources)) {
      if (src.origin !== 'generated' || this.buffers.has(src.id) || this.pending.has(src.id)) continue;
      this.pending.add(src.id);
      const ctx = this.ctx;
      // Generation is synchronous DSP; yield first so the UI paints.
      setTimeout(() => {
        this.pending.delete(src.id);
        this.put(src.id, generateBuffer(ctx, src.seed ?? 1, src.waveKind ?? 'tonal', src.durationSeconds));
      }, 0);
    }
  }

  /** Peak envelope (max abs per column) for the whole source, PEAKS_PER_SECOND columns per second. */
  peaksFor(id: string): Float32Array | null {
    const cached = this.peaks.get(id);
    if (cached) return cached;
    const buffer = this.buffers.get(id);
    if (!buffer) return null;
    const cols = Math.max(1, Math.ceil(buffer.duration * PEAKS_PER_SECOND));
    const out = new Float32Array(cols);
    const perCol = buffer.sampleRate / PEAKS_PER_SECOND;
    for (let c = 0; c < buffer.numberOfChannels; c++) {
      const data = buffer.getChannelData(c);
      for (let i = 0; i < cols; i++) {
        const a = Math.floor(i * perCol);
        const b = Math.min(data.length, Math.floor((i + 1) * perCol));
        let peak = 0;
        for (let j = a; j < b; j++) {
          const v = Math.abs(data[j]!);
          if (v > peak) peak = v;
        }
        if (peak > out[i]!) out[i] = peak;
      }
    }
    this.peaks.set(id, out);
    return out;
  }

  async decode(data: ArrayBuffer): Promise<AudioBuffer> {
    if (!this.ctx) throw new Error('AudioLibrary: no audio context attached');
    return this.ctx.decodeAudioData(data.slice(0));
  }
}

export const library = new AudioLibrary();
