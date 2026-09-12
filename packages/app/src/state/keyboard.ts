import { useEffect } from 'react';
import { commands } from '@ondera/core';
import { useStore } from './session';

/** Global transport shortcuts. Everything goes through commands. */
export function useKeyboardShortcuts(): void {
  const store = useStore();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      switch (e.code) {
        case 'Space':
          e.preventDefault();
          store.dispatch(commands.transport.togglePlay({}));
          break;
        case 'Home':
        case 'Enter':
          e.preventDefault();
          store.dispatch(commands.transport.returnToStart({}));
          break;
        case 'KeyC':
          store.dispatch(commands.transport.setCycle({ enabled: !store.getState().transport.cycle }));
          break;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [store]);
}
