import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  KeybindingStore,
  chordToString,
  formatChord,
  DEFAULT_SHORTCUTS,
  type ActionId,
} from './keybinding-store';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
});

function keydown(
  key: string,
  opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {}
) {
  return {
    key,
    type: 'keydown' as const,
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
  };
}

const SPLIT_ACTION_IDS: ActionId[] = [
  'split.focusLeft',
  'split.focusRight',
  'split.focusUp',
  'split.focusDown',
  'split.resizeLeft',
  'split.resizeRight',
  'split.resizeUp',
  'split.resizeDown',
  'split.zoom',
  'split.swapPanes',
  'split.rotateSplit',
];

describe('Split shortcut registration', () => {
  beforeEach(() => {
    storage.clear();
  });

  it('all new split shortcuts are registered in DEFAULT_SHORTCUTS', () => {
    const registeredIds = DEFAULT_SHORTCUTS.map(s => s.id);
    for (const id of SPLIT_ACTION_IDS) {
      expect(registeredIds).toContain(id);
    }
  });

  it('all new split shortcuts are in the Split category', () => {
    for (const id of SPLIT_ACTION_IDS) {
      const def = DEFAULT_SHORTCUTS.find(s => s.id === id);
      expect(def).toBeDefined();
      expect(def!.category).toBe('Split');
    }
  });

  it('all new split shortcuts have type app', () => {
    for (const id of SPLIT_ACTION_IDS) {
      const def = DEFAULT_SHORTCUTS.find(s => s.id === id);
      expect(def).toBeDefined();
      expect(def!.type).toBe('app');
    }
  });

  it('no duplicate action IDs in DEFAULT_SHORTCUTS', () => {
    const ids = DEFAULT_SHORTCUTS.map(s => s.id);
    const unique = new Set(ids);
    expect(unique.size).toBe(ids.length);
  });

  it('no conflicting default chords in DEFAULT_SHORTCUTS', () => {
    const chords = new Map<string, ActionId>();
    for (const def of DEFAULT_SHORTCUTS) {
      const str = chordToString(def.defaultChord);
      const existing = chords.get(str);
      expect(existing).toBeUndefined();
      chords.set(str, def.id);
    }
  });
});

describe('Split shortcut matching', () => {
  beforeEach(() => {
    storage.clear();
  });

  it('matches Ctrl+ArrowLeft to split.focusLeft', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowLeft', { ctrlKey: true }))).toBe('split.focusLeft');
  });

  it('matches Ctrl+ArrowRight to split.focusRight', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowRight', { ctrlKey: true }))).toBe('split.focusRight');
  });

  it('matches Ctrl+ArrowUp to split.focusUp', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowUp', { ctrlKey: true }))).toBe('split.focusUp');
  });

  it('matches Ctrl+ArrowDown to split.focusDown', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowDown', { ctrlKey: true }))).toBe('split.focusDown');
  });

  it('matches Ctrl+Alt+ArrowLeft to split.resizeLeft', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowLeft', { ctrlKey: true, altKey: true }))).toBe('split.resizeLeft');
  });

  it('matches Ctrl+Alt+ArrowRight to split.resizeRight', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowRight', { ctrlKey: true, altKey: true }))).toBe('split.resizeRight');
  });

  it('matches Ctrl+Alt+ArrowUp to split.resizeUp', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowUp', { ctrlKey: true, altKey: true }))).toBe('split.resizeUp');
  });

  it('matches Ctrl+Alt+ArrowDown to split.resizeDown', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowDown', { ctrlKey: true, altKey: true }))).toBe('split.resizeDown');
  });

  it('matches Ctrl+Shift+Z to split.zoom', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('Z', { ctrlKey: true, shiftKey: true }))).toBe('split.zoom');
  });

  it('matches Ctrl+Shift+X to split.swapPanes', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe('split.swapPanes');
  });

  it('matches Ctrl+Shift+R to split.rotateSplit', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('R', { ctrlKey: true, shiftKey: true }))).toBe('split.rotateSplit');
  });
});

describe('Split shortcut non-conflicts', () => {
  beforeEach(() => {
    storage.clear();
  });

  it('Ctrl+Z (suspend) does not conflict with Ctrl+Shift+Z (zoom)', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('z', { ctrlKey: true }))).toBe('terminal.suspend');
    expect(store.matchAction(keydown('Z', { ctrlKey: true, shiftKey: true }))).toBe('split.zoom');
  });

  it('Ctrl+Arrow focus shortcuts do not conflict with Ctrl+Alt+Arrow resize shortcuts', () => {
    const store = new KeybindingStore();
    expect(store.matchAction(keydown('ArrowLeft', { ctrlKey: true }))).toBe('split.focusLeft');
    expect(store.matchAction(keydown('ArrowLeft', { ctrlKey: true, altKey: true }))).toBe('split.resizeLeft');
  });

  it('all new split shortcuts are classified as app shortcuts', () => {
    const store = new KeybindingStore();
    for (const id of SPLIT_ACTION_IDS) {
      expect(store.isAppShortcut(keydown('dummy'))).toBe(false); // sanity check
    }

    // Check each shortcut is classified correctly
    expect(store.isAppShortcut(keydown('ArrowLeft', { ctrlKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowRight', { ctrlKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowUp', { ctrlKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowDown', { ctrlKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowLeft', { ctrlKey: true, altKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowRight', { ctrlKey: true, altKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowUp', { ctrlKey: true, altKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('ArrowDown', { ctrlKey: true, altKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('Z', { ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(store.isAppShortcut(keydown('R', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('none of the new shortcuts conflict with existing ones', () => {
    const store = new KeybindingStore();
    for (const id of SPLIT_ACTION_IDS) {
      const def = DEFAULT_SHORTCUTS.find(s => s.id === id)!;
      const conflict = store.findConflict(def.defaultChord, id);
      expect(conflict).toBeNull();
    }
  });
});

describe('Split shortcut display formatting', () => {
  it('formats arrow key shortcuts with arrow symbols', () => {
    const leftDef = DEFAULT_SHORTCUTS.find(s => s.id === 'split.focusLeft')!;
    expect(formatChord(leftDef.defaultChord)).toBe('Ctrl+\u2190');
  });

  it('formats resize shortcuts with Ctrl+Alt+arrow', () => {
    const resizeDef = DEFAULT_SHORTCUTS.find(s => s.id === 'split.resizeRight')!;
    expect(formatChord(resizeDef.defaultChord)).toBe('Ctrl+Alt+\u2192');
  });

  it('formats zoom shortcut as Ctrl+Shift+Z', () => {
    const zoomDef = DEFAULT_SHORTCUTS.find(s => s.id === 'split.zoom')!;
    expect(formatChord(zoomDef.defaultChord)).toBe('Ctrl+Shift+Z');
  });

  it('formats swap shortcut as Ctrl+Shift+X', () => {
    const swapDef = DEFAULT_SHORTCUTS.find(s => s.id === 'split.swapPanes')!;
    expect(formatChord(swapDef.defaultChord)).toBe('Ctrl+Shift+X');
  });

  it('formats rotate shortcut as Ctrl+Shift+R', () => {
    const rotateDef = DEFAULT_SHORTCUTS.find(s => s.id === 'split.rotateSplit')!;
    expect(formatChord(rotateDef.defaultChord)).toBe('Ctrl+Shift+R');
  });
});
