import '@fontsource/manrope/400.css';
import '@fontsource/manrope/500.css';
import '@fontsource/manrope/600.css';
import '@fontsource/manrope/700.css';
import '@fontsource/ibm-plex-mono/400.css';
import '@fontsource/ibm-plex-mono/500.css';
import './theme/global.css';
import './theme/materials.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { SessionStore, attachEngine, commands, createMockSession } from '@ondera/core';
import { applyTokens } from './theme/applyTokens';
import { SessionProvider } from './state/session';
import { attachDocument } from './state/document';
import { attachRecorder } from './audio/recorder';
import { engine } from './audio/instance';
import { App } from './App';

applyTokens();

const store = new SessionStore(createMockSession());
attachEngine(store, engine);
attachDocument(store);
attachRecorder(store);

// Dev console access to the command layer: `onderaDebug.store.dispatch(...)`. The CLI/MCP will replace this.
Object.assign(window, { onderaDebug: { store, engine, commands } });

// Browsers only start audio after a gesture; Electron does not mind the extra resume.
const wake = () => engine.resume();
window.addEventListener('pointerdown', wake, { capture: true });
window.addEventListener('keydown', wake, { capture: true });

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <SessionProvider store={store}>
      <App />
    </SessionProvider>
  </StrictMode>,
);
