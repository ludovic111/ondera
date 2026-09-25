// Presentation commands retain the existing controls. NativeStore translates them
// to the Rust registry; there is no JavaScript document reducer or audio engine.
// Every name here is a registry command or a `case` in NativeStore.translate:
// engine/tests/parity.rs checks it.
export * from "./types";
export * from "./time";
export * from "./gain";
export * from "./fade";
export * from "./strip";
export * from "./palette";
export type { NativeStore as SessionStore } from "../state/native";
export interface Command<P = Record<string, unknown>> {
  name: string;
  params: P;
}
const command =
  (name: string) =>
  (params: Record<string, unknown>): Command => ({ name, params });
export const ZOOM_MIN_PX_PER_BAR = 12;
export const ZOOM_MAX_PX_PER_BAR = 480;
export const INSERT_STATES = ["empty", "active", "bypassed"] as const;
export const commands = {
  history: {
    undo: command("history.undo"),
    redo: command("history.redo"),
  },
  note: {
    add: command("note.add"),
    update: command("note.update"),
    remove: command("note.remove"),
    select: command("note.select"),
  },
  controller: {
    add: command("controller.add"),
    update: command("controller.update"),
    remove: command("controller.remove"),
    setPoints: command("controller.setPoints"),
  },
  clip: {
    select: command("clip.select"),
    clearSelection: command("clip.clearSelection"),
    rename: command("clip.rename"),
    create: command("clip.create"),
    move: command("clip.move"),
    resize: command("clip.resize"),
    split: command("clip.split"),
    duplicate: command("clip.duplicate"),
    remove: command("clip.remove"),
    setFades: command("clip.setFades"),
    setGain: command("clip.setGain"),
  },
  marker: {
    add: command("marker.add"),
    rename: command("marker.rename"),
    move: command("marker.move"),
    remove: command("marker.remove"),
    goto: command("marker.goto"),
    next: command("marker.next"),
    previous: command("marker.previous"),
    cycleSection: command("marker.cycleSection"),
  },
  strip: {
    setSendLevel: command("strip.setSendLevel"),
    setInsertState: command("strip.setInsertState"),
    setInstrument: command("strip.setInstrument"),
    setInsert: command("strip.setInsert"),
  },
  agent: {
    setDraft: command("agent.setDraft"),
    stopCurrent: command("agent.stopCurrent"),
    submit: command("agent.submit"),
  },
  view: {
    setZoom: command("view.setZoom"),
    zoomBy: command("view.zoomBy"),
    scrollTo: command("view.scrollTo"),
    scrollBy: command("view.scrollBy"),
    setAgentPanelOpen: command("view.setAgentPanelOpen"),
    setBrowserTab: command("view.setBrowserTab"),
    setEditorMode: command("view.setEditorMode"),
    setArrangeTool: command("view.setArrangeTool"),
    setEditorClip: command("view.setEditorClip"),
    setBrowserSelection: command("view.setBrowserSelection"),
    setFollowPlayhead: command("view.setFollowPlayhead"),
  },
  session: {
    rename: command("session.rename"),
  },
  track: {
    setMute: command("track.setMute"),
    setSolo: command("track.setSolo"),
    setArmed: command("track.setArmed"),
    setMonitor: command("track.setMonitor"),
    setVolume: command("track.setVolume"),
    setPan: command("track.setPan"),
    rename: command("track.rename"),
    select: command("track.select"),
    setColor: command("track.setColor"),
    add: command("track.add"),
    remove: command("track.remove"),
    move: command("track.move"),
    duplicate: command("track.duplicate"),
  },
  transport: {
    play: command("transport.play"),
    stop: command("transport.stop"),
    togglePlay: command("transport.togglePlay"),
    returnToStart: command("transport.returnToStart"),
    nudge: command("transport.nudge"),
    setPosition: command("transport.setPosition"),
    setRecording: command("transport.setRecording"),
    setCycle: command("transport.setCycle"),
    setCycleRange: command("transport.setCycleRange"),
    setTempo: command("transport.setTempo"),
    setMetronome: command("transport.setMetronome"),
    setSnap: command("transport.setSnap"),
    setTimeSignature: command("transport.setTimeSignature"),
    setKey: command("transport.setKey"),
  },
};
