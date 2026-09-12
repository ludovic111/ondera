import type { EngineClient } from './EngineClient';

/** Core has no DOM or Node lib; the host is expected to provide timers. */
interface HostTimers {
  setInterval(fn: () => void, ms: number): unknown;
  clearInterval(handle: unknown): void;
  performance?: { now(): number };
}
const host = globalThis as unknown as HostTimers;

export interface MockEngineOptions {
  /** Tick interval in ms. ~60 Hz by default. */
  intervalMs?: number;
  now?: () => number;
  setInterval?: (fn: () => void, ms: number) => unknown;
  clearInterval?: (handle: unknown) => void;
}

/** Advances the playhead on a wall-clock timer. No audio. */
export class MockEngine implements EngineClient {
  private positionBeats = 0;
  private tempo = 120;
  private playing = false;
  private startedAt = 0;
  private startBeats = 0;
  private handle: unknown = null;
  private listeners = new Set<(beats: number) => void>();
  private readonly intervalMs: number;
  private readonly now: () => number;
  private readonly setIntervalFn: (fn: () => void, ms: number) => unknown;
  private readonly clearIntervalFn: (handle: unknown) => void;

  constructor(opts: MockEngineOptions = {}) {
    this.intervalMs = opts.intervalMs ?? 1000 / 60;
    this.now = opts.now ?? (() => host.performance?.now() ?? Date.now());
    this.setIntervalFn = opts.setInterval ?? ((fn, ms) => host.setInterval(fn, ms));
    this.clearIntervalFn = opts.clearInterval ?? ((h) => host.clearInterval(h));
  }

  start(fromBeats: number, tempo: number): void {
    this.tempo = tempo;
    this.startBeats = fromBeats;
    this.positionBeats = fromBeats;
    this.startedAt = this.now();
    this.playing = true;
    if (this.handle === null) this.handle = this.setIntervalFn(this.tick, this.intervalMs);
  }

  stop(): void {
    this.playing = false;
    if (this.handle !== null) {
      this.clearIntervalFn(this.handle);
      this.handle = null;
    }
  }

  locate(beats: number): void {
    this.positionBeats = beats;
    this.startBeats = beats;
    this.startedAt = this.now();
  }

  setTempo(bpm: number): void {
    // Re-anchor so the tempo change does not jump the playhead.
    this.startBeats = this.positionBeats;
    this.startedAt = this.now();
    this.tempo = bpm;
  }

  onTick(cb: (beats: number) => void): () => void {
    this.listeners.add(cb);
    return () => {
      this.listeners.delete(cb);
    };
  }

  dispose(): void {
    this.stop();
    this.listeners.clear();
  }

  private tick = () => {
    if (!this.playing) return;
    const elapsedSec = (this.now() - this.startedAt) / 1000;
    this.positionBeats = this.startBeats + elapsedSec * (this.tempo / 60);
    for (const l of this.listeners) l(this.positionBeats);
  };
}
