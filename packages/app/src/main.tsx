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
import { MockEngine, SessionStore, attachEngine, createMockSession } from '@ondera/core';
import { applyTokens } from './theme/applyTokens';
import { SessionProvider } from './state/session';
import { App } from './App';

applyTokens();

const store = new SessionStore(createMockSession());
attachEngine(store, new MockEngine());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <SessionProvider store={store}>
      <App />
    </SessionProvider>
  </StrictMode>,
);
