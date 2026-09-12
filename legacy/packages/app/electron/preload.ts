import { contextBridge, ipcRenderer } from 'electron';

export interface ImportedFile {
  name: string;
  data: ArrayBuffer;
}

/**
 * The renderer is sandboxed; everything that touches disk goes through here.
 * Sessions are single `.ondera` JSON files (audio embedded as base64 WAV).
 */
const api = {
  platform: process.platform,
  fs: {
    openSession: (): Promise<{ path: string; json: string } | null> => ipcRenderer.invoke('session:open'),
    saveSession: (path: string | null, defaultName: string, json: string): Promise<string | null> =>
      ipcRenderer.invoke('session:save', path, defaultName, json),
    importAudio: (): Promise<ImportedFile[]> => ipcRenderer.invoke('audio:import'),
    saveFile: (defaultName: string, data: ArrayBuffer, filterName: string, extensions: string[]): Promise<boolean> =>
      ipcRenderer.invoke('file:save', defaultName, data, filterName, extensions),
  },
};

export type OnderaBridge = typeof api;

contextBridge.exposeInMainWorld('ondera', api);
