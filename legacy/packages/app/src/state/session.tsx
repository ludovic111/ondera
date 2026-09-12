import { createContext, useCallback, useContext, useSyncExternalStore, type ReactNode } from 'react';
import type { Command, Session, SessionStore } from '@ondera/core';

const StoreContext = createContext<SessionStore | null>(null);

export function SessionProvider({ store, children }: { store: SessionStore; children: ReactNode }) {
  return <StoreContext.Provider value={store}>{children}</StoreContext.Provider>;
}

export function useStore(): SessionStore {
  const store = useContext(StoreContext);
  if (!store) throw new Error('useStore must be used inside <SessionProvider>');
  return store;
}

/**
 * Read a slice of session state. The selector must return a stable value
 * (a primitive or an object already held in state) or the component will
 * re-render on every dispatch.
 */
export function useSession<T>(selector: (s: Session) => T): T {
  const store = useStore();
  return useSyncExternalStore(
    store.subscribe,
    () => selector(store.getState()),
    () => selector(store.getState()),
  );
}

/** The only way a component changes session state. */
export function useDispatch(): (command: Command<any>) => void {
  const store = useStore();
  return useCallback(
    (command: Command<any>) => {
      store.dispatch(command);
    },
    [store],
  );
}
