import type { Session, SessionStore } from '@ondera/core';
import { actions, runAction, type ActionId } from './actions';
import { formatShortcut } from './shortcuts';
import { bounceSession, importAudioFiles, newSession, openSession, saveSession } from './document';

export interface MenuItem {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  checked?: boolean;
  onSelect?: () => void;
  separator?: false;
}

export type MenuEntry = MenuItem | { separator: true };

export const separator: MenuEntry = { separator: true };

/** A menu row for an action, resolving enabled/checked against current state. */
export function actionItem(store: SessionStore, id: ActionId, label?: string): MenuEntry {
  const def = actions[id];
  const state = store.getState();
  return {
    label: label ?? def.label,
    ...(def.shortcut ? { shortcut: formatShortcut(def.shortcut) } : {}),
    disabled: def.enabled ? !def.enabled(state, store) : false,
    checked: def.checked ? def.checked(state) : false,
    onSelect: () => runAction(store, id),
  };
}

/** Inert row: something the design shows but Phase 1 does not implement. */
export const later = (label: string): MenuEntry => ({ label, disabled: true });

export const MENU_TITLES = ['File', 'Edit', 'Track', 'Mix', 'Agent', 'View', 'Help'] as const;
export type MenuTitle = (typeof MENU_TITLES)[number];

export function buildMenu(title: MenuTitle, store: SessionStore): MenuEntry[] {
  const a = (id: ActionId, label?: string) => actionItem(store, id, label);
  switch (title) {
    case 'File':
      return [
        { label: 'New Session', shortcut: '⌘N', onSelect: () => newSession(store) },
        { label: 'Open…', shortcut: '⌘O', onSelect: () => void openSession(store) },
        separator,
        { label: 'Save', shortcut: '⌘S', onSelect: () => void saveSession(store) },
        { label: 'Save As…', shortcut: '⇧⌘S', onSelect: () => void saveSession(store, true) },
        separator,
        { label: 'Import Audio…', shortcut: '⌘I', onSelect: () => void importAudioFiles(store) },
        { label: 'Bounce Mix to WAV…', shortcut: '⌘B', onSelect: () => void bounceSession(store) },
      ];
    case 'Edit':
      return [
        a('undo'),
        a('redo'),
        separator,
        a('deleteSelection'),
        a('duplicateClip'),
        a('splitAtPlayhead'),
        separator,
        a('openInEditor'),
      ];
    case 'Track':
      return [
        a('addAudioTrack'),
        a('addMidiTrack'),
        separator,
        a('muteSelectedTrack'),
        a('soloSelectedTrack'),
        a('armSelectedTrack'),
        separator,
        a('removeSelectedTrack'),
      ];
    case 'Mix':
      return [a('metronome'), a('cycle'), a('record'), separator, later('Mixer'), later('Master Bus')];
    case 'Agent':
      return [a('toggleAgentPanel', 'Show Agent Panel'), a('stopAgent'), separator, later('Connect MCP Server…')];
    case 'View':
      return [
        a('zoomIn'),
        a('zoomOut'),
        a('zoomToFit'),
        separator,
        a('followPlayhead'),
        separator,
        a('editorPianoRoll'),
        a('editorScore'),
        a('editorStep'),
        separator,
        a('toolPointer'),
        a('toolPencil'),
        a('toolScissors'),
        a('toolGrid'),
      ];
    case 'Help':
      return [later('Ondera 0.0.1 · Phase 1 UI shell'), later('Keyboard shortcuts are listed in the menus')];
  }
}

export type { Session };
