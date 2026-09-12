import type { Meters, Session } from '../model/types';

/**
 * The boundary between core and the audio engine process. In Phase 1 the
 * only implementation is MockEngine. The Rust process will sit behind the
 * same interface, driven over IPC.
 */
export interface EngineClient {
  /** Begin advancing the playhead from `fromBeats`. */
  start(fromBeats: number, tempo: number): void;
  stop(): void;
  /** Reposition while stopped or playing. */
  locate(beats: number): void;
  setTempo(bpm: number): void;
  /** Called with the current position while playing. */
  onTick(cb: (beats: number) => void): () => void;
  /**
   * Hand the engine the session after every non-transient change so it can
   * mirror tracks, clips, mix settings and the cycle range. Optional: the
   * MockEngine has nothing to render.
   */
  sync?(state: Session): void;
  /** Meter readings, 0..1 per side, pushed while the engine runs. */
  onMeters?(cb: (meters: Meters) => void): () => void;
  /** Audition a pitch on a track's instrument (piano-roll clicks). */
  previewNote?(trackId: string, pitch: number, velocity: number): void;
  dispose(): void;
}
