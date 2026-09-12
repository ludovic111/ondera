/**
 * Microphone recording. Watches the transport: when it runs with record
 * enabled and at least one audio track armed, capture starts; when it stops
 * (or record is switched off), the take is decoded and committed as clips.
 */
import type { SessionStore } from '@ondera/core';
import { beatsToBars } from '@ondera/core';
import { library } from './library';
import { commitRecording } from '../state/document';

let takeCount = 0;

export function attachRecorder(store: SessionStore): () => void {
  let recorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let startBar = 0;
  let active = false;

  const shouldRecord = () => {
    const s = store.getState();
    return s.transport.playing && s.transport.recording && s.tracks.some((t) => t.kind === 'audio' && t.armed);
  };

  const start = async () => {
    active = true;
    const s = store.getState();
    startBar = Math.max(0, Math.round(beatsToBars(s.transport.positionBeats, s.transport.timeSignature) * 16) / 16);
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false } });
    } catch {
      active = false;
      window.alert('Ondera could not access a microphone. Check the input permission and try again.');
      return;
    }
    if (!shouldRecord()) {
      stream.getTracks().forEach((t) => t.stop());
      stream = null;
      active = false;
      return;
    }
    const chunks: BlobPart[] = [];
    recorder = new MediaRecorder(stream);
    recorder.ondataavailable = (e) => {
      if (e.data.size) chunks.push(e.data);
    };
    recorder.onstop = async () => {
      stream?.getTracks().forEach((t) => t.stop());
      stream = null;
      recorder = null;
      if (!chunks.length) return;
      const blob = new Blob(chunks);
      try {
        const buffer = await library.decode(await blob.arrayBuffer());
        takeCount += 1;
        commitRecording(store, buffer, startBar, `Take ${takeCount}`);
      } catch {
        window.alert('The recording could not be decoded.');
      }
    };
    recorder.start(250);
  };

  const stop = () => {
    active = false;
    if (recorder && recorder.state !== 'inactive') recorder.stop();
  };

  const off = store.subscribe((_s, cmd) => {
    if (cmd.name === 'transport.tick' || cmd.name === 'session.updateMeters') return;
    const want = shouldRecord();
    if (want && !active) void start();
    else if (!want && active) stop();
  });
  return () => {
    off();
    stop();
  };
}
