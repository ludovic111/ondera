import { contextBridge } from 'electron';

/** Minimal bridge. Phase 1 only needs to know the platform for title-bar layout. */
const api = {
  platform: process.platform,
};

export type OnderaBridge = typeof api;

contextBridge.exposeInMainWorld('ondera', api);
