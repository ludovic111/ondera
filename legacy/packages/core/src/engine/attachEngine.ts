import type { EngineClient } from './EngineClient';
import type { SessionStore } from '../store';
import { commands } from '../registry';

/**
 * Wires a store to an engine. Transport commands drive the engine; engine
 * ticks come back as transient transport.tick commands. The engine is just
 * another client of the command layer.
 */
export function attachEngine(store: SessionStore, engine: EngineClient): () => void {
  let wasPlaying = store.getState().transport.playing;
  let lastTempo = store.getState().transport.tempo;

  const offTick = engine.onTick((beats) => {
    store.dispatch(commands.transport.tick({ beats }));
  });
  const offMeters = engine.onMeters?.((m) => {
    store.dispatch(commands.session.updateMeters(m));
  });

  engine.sync?.(store.getState());

  const offStore = store.subscribe((state, command) => {
    const t = state.transport;
    if (command.name !== 'transport.tick' && command.name !== 'session.updateMeters') engine.sync?.(state);
    if (t.playing !== wasPlaying) {
      wasPlaying = t.playing;
      if (t.playing) engine.start(t.positionBeats, t.tempo);
      else engine.stop();
    } else if (command.name !== 'transport.tick' && command.name.startsWith('transport.')) {
      // A locate while stopped (or a jump while playing) that did not come from the engine.
      engine.locate(t.positionBeats);
    }
    if (t.tempo !== lastTempo) {
      lastTempo = t.tempo;
      engine.setTempo(t.tempo);
    }
  });

  if (wasPlaying) {
    const t = store.getState().transport;
    engine.start(t.positionBeats, t.tempo);
  }

  return () => {
    offTick();
    offMeters?.();
    offStore();
    engine.stop();
  };
}
