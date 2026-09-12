import type { OnderaBridge } from '../electron/preload';

declare global {
  interface Window {
    ondera?: OnderaBridge;
  }
}

export {};
