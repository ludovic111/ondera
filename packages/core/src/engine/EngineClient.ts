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
  dispose(): void;
}
