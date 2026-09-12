/**
 * The document layer: new / open / save / import / bounce. Sits between the
 * host (Electron dialogs, or browser downloads when there is no bridge) and
 * the store. Everything that changes the session still goes through commands;
 * loading a whole session goes through the store's staged session.load.
 */
import {
  commands,
  createEmptySession,
  getCommandDef,
  secondsToBars,
  type AudioSource,
  type Session,
  type SessionStore,
} from '@ondera/core';
import { library } from '../audio/library';
import { encodeWav } from '../audio/wav';
import { bounce } from '../audio/bounce';
import { newId } from './ids';

const FORMAT = 'ondera-session';
const VERSION = 1;

interface SessionFile {
  format: typeof FORMAT;
  version: number;
  session: Session;
  /** Base64 WAV per non-generated source id. */
  audio: Record<string, string>;
}

let currentPath: string | null = null;
let dirty = false;
const listeners = new Set<() => void>();

export function documentPath(): string | null {
  return currentPath;
}
export function isDirty(): boolean {
  return dirty;
}
export function onDocumentChange(cb: () => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
function notify(): void {
  for (const l of listeners) l();
}

/** Track the dirty flag: any recorded (non-transient) command dirties the document. */
export function attachDocument(store: SessionStore): () => void {
  const off = store.subscribe((state, command) => {
    if (command.name === 'session.load') {
      dirty = false;
    } else if (!isTransientName(command.name)) {
      dirty = true;
    }
    document.title = `${state.name}${dirty ? ' — edited' : ''}`;
    notify();
  });
  document.title = store.getState().name;
  return off;
}

/** Undo and redo change the document even though they are not recorded themselves. */
function isTransientName(name: string): boolean {
  if (name === 'history.undo' || name === 'history.redo') return false;
  return getCommandDef(name).transient;
}

const bridge = () => window.ondera?.fs;

function toBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}
function fromBase64(b64: string): ArrayBuffer {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out.buffer;
}

/** The `.ondera` file: session JSON plus base64 WAV for imported and recorded sources. */
export function serializeSession(state: Session): string {
  const audio: Record<string, string> = {};
  for (const src of Object.values(state.sources)) {
    if (src.origin === 'generated') continue;
    const buffer = library.get(src.id);
    if (buffer) audio[src.id] = toBase64(encodeWav(buffer));
  }
  const stopped: Session = {
    ...state,
    transport: { ...state.transport, playing: false, recording: false },
    meters: { masterL: 0, masterR: 0, cpu: 0, channelL: 0, channelR: 0 },
  };
  const file: SessionFile = { format: FORMAT, version: VERSION, session: stopped, audio };
  return JSON.stringify(file);
}

export async function loadSessionJson(store: SessionStore, json: string): Promise<void> {
  const file = JSON.parse(json) as SessionFile;
  if (file.format !== FORMAT) throw new Error('Not an Ondera session file');
  for (const [id, b64] of Object.entries(file.audio ?? {})) {
    library.put(id, await library.decode(fromBase64(b64)));
  }
  store.dispatch(commands.session.load({ token: store.stage(file.session) }));
}

function download(name: string, data: BlobPart, type: string): void {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function pickFiles(accept: string, multiple: boolean): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.multiple = multiple;
    input.onchange = () => resolve(Array.from(input.files ?? []));
    input.oncancel = () => resolve([]);
    input.click();
  });
}

export function newSession(store: SessionStore): void {
  if (dirty && !window.confirm('Discard unsaved changes and start a new session?')) return;
  store.dispatch(commands.transport.stop({}));
  currentPath = null;
  store.dispatch(commands.session.load({ token: store.stage(createEmptySession()) }));
}

export async function openSession(store: SessionStore): Promise<void> {
  if (dirty && !window.confirm('Discard unsaved changes and open another session?')) return;
  const fs = bridge();
  let json: string | null = null;
  if (fs) {
    const result = await fs.openSession();
    if (!result) return;
    currentPath = result.path;
    json = result.json;
  } else {
    const [file] = await pickFiles('.ondera,application/json', false);
    if (!file) return;
    json = await file.text();
    currentPath = null;
  }
  if (store.getState().transport.playing) store.dispatch(commands.transport.stop({}));
  await loadSessionJson(store, json);
}

export async function saveSession(store: SessionStore, saveAs = false): Promise<void> {
  const state = store.getState();
  const json = serializeSession(state);
  const fs = bridge();
  if (fs) {
    const path = await fs.saveSession(saveAs ? null : currentPath, state.name, json);
    if (!path) return;
    currentPath = path;
    const base = path.split(/[\\/]/).pop() ?? state.name;
    if (base !== state.name) store.dispatch(commands.session.rename({ name: base }));
  } else {
    download(state.name.endsWith('.ondera') ? state.name : `${state.name}.ondera`, json, 'application/json');
  }
  dirty = false;
  document.title = store.getState().name;
  notify();
}

export interface ImportTarget {
  trackId: string | null;
  startBar: number;
}

/** Decode audio files, register them as sources and lay them out as clips. */
export async function importAudioFiles(store: SessionStore, files?: File[], target?: ImportTarget): Promise<void> {
  let inputs: { name: string; data: ArrayBuffer }[] = [];
  if (files) {
    inputs = await Promise.all(files.map(async (f) => ({ name: f.name, data: await f.arrayBuffer() })));
  } else if (bridge()) {
    inputs = await bridge()!.importAudio();
  } else {
    const picked = await pickFiles('audio/*', true);
    inputs = await Promise.all(picked.map(async (f) => ({ name: f.name, data: await f.arrayBuffer() })));
  }
  if (!inputs.length) return;

  const state = store.getState();
  let trackId = target?.trackId ?? state.view.selectedTrackId;
  let track = state.tracks.find((t) => t.id === trackId);
  if (!track || track.kind !== 'audio') {
    // No audio track under the drop: make one.
    trackId = newId('track');
    store.dispatch(commands.track.add({ trackId, kind: 'audio', name: inputs[0]!.name.replace(/\.[^.]+$/, '') }));
    track = store.getState().tracks.find((t) => t.id === trackId);
  }
  let bar = target?.startBar ?? Math.round(store.getState().transport.positionBeats / 4);
  for (const input of inputs) {
    let buffer: AudioBuffer;
    try {
      buffer = await library.decode(input.data);
    } catch {
      window.alert(`Could not decode ${input.name}`);
      continue;
    }
    const sourceId = newId('src');
    library.put(sourceId, buffer);
    const s = store.getState();
    const name = input.name.replace(/\.[^.]+$/, '');
    store.dispatch(
      commands.session.addSource({
        sourceId,
        name,
        durationSeconds: buffer.duration,
        sampleRate: buffer.sampleRate,
        channels: Math.min(2, buffer.numberOfChannels),
        origin: 'file',
        fileName: input.name,
      }),
    );
    const lengthBars = Math.max(0.25, secondsToBars(buffer.duration, s.transport.tempo, s.transport.timeSignature));
    store.dispatch(commands.clip.create({ clipId: newId('clip'), trackId: trackId!, startBar: bar, lengthBars, name, sourceId }));
    bar += Math.ceil(lengthBars);
  }
}

/** Register a captured recording as a source and place clips on the armed audio tracks. */
export function commitRecording(store: SessionStore, buffer: AudioBuffer, startBar: number, name: string): void {
  const s = store.getState();
  const armed = s.tracks.filter((t) => t.kind === 'audio' && t.armed);
  if (!armed.length) return;
  const sourceId = newId('src');
  library.put(sourceId, buffer);
  store.dispatch(
    commands.session.addSource({
      sourceId,
      name,
      durationSeconds: buffer.duration,
      sampleRate: buffer.sampleRate,
      channels: Math.min(2, buffer.numberOfChannels),
      origin: 'recording',
      fileName: `${name}.wav`,
    }),
  );
  const lengthBars = Math.max(0.25, secondsToBars(buffer.duration, s.transport.tempo, s.transport.timeSignature));
  for (const t of armed) {
    store.dispatch(commands.clip.create({ clipId: newId('clip'), trackId: t.id, startBar, lengthBars, name, sourceId }));
  }
}

export async function bounceSession(store: SessionStore): Promise<void> {
  const state = store.getState();
  if (state.transport.playing) store.dispatch(commands.transport.stop({}));
  const wav = await bounce(state);
  const name = `${state.name.replace(/\.ondera$/, '')}.wav`;
  const fs = bridge();
  if (fs) await fs.saveFile(name, wav, 'WAV audio', ['wav']);
  else download(name, wav, 'audio/wav');
}

export type { AudioSource };
