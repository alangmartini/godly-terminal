import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  KeybindingStore,
  chordToString,
  eventToChord,
  formatChord,
  DEFAULT_SHORTCUTS,
  type KeyChord,
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

describe('KeybindingStore', () => {
  beforeEach(() => {
    storage.clear();
  });

  describe('default bindings', () => {
    it('has a binding for every action in DEFAULT_SHORTCUTS', () => {
      const store = new KeybindingStore();
      for (const def of DEFAULT_SHORTCUTS) {
        expect(store.getBinding(def.id)).toEqual(def.defaultChord);
      }
    });

    it('reports no custom bindings initially', () => {
      const store = new KeybindingStore();
      for (const def of DEFAULT_SHORTCUTS) {
        expect(store.isCustom(def.id)).toBe(false);
      }
    });
  });

  describe('matchAction', () => {
    it('matches Ctrl+C to terminal.interrupt', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('c', { ctrlKey: true }))).toBe('terminal.interrupt');
    });

    it('matches Ctrl+Shift+C to clipboard.copy', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('C', { ctrlKey: true, shiftKey: true }))).toBe(
        'clipboard.copy'
      );
    });

    it('matches Ctrl+T to tabs.newTerminal', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('t', { ctrlKey: true }))).toBe('tabs.newTerminal');
    });

    it('matches Ctrl+Tab to tabs.nextTab', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('Tab', { ctrlKey: true }))).toBe('tabs.nextTab');
    });

    it('matches Ctrl+Shift+Tab to tabs.previousTab', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('Tab', { ctrlKey: true, shiftKey: true }))).toBe(
        'tabs.previousTab'
      );
    });

    it('returns null for unbound keys', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('a', { ctrlKey: true }))).toBeNull();
    });

    it('returns null for keyup events', () => {
      const store = new KeybindingStore();
      expect(
        store.matchAction({ key: 'c', ctrlKey: true, shiftKey: false, altKey: false, type: 'keyup' })
      ).toBeNull();
    });
  });

  describe('setBinding', () => {
    it('changes the action binding and rebuilds the index', () => {
      const store = new KeybindingStore();
      const newChord: KeyChord = { ctrlKey: true, shiftKey: true, altKey: false, key: 'x' };
      store.setBinding('terminal.interrupt', newChord);

      expect(store.getBinding('terminal.interrupt')).toEqual(newChord);
      expect(store.matchAction(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe(
        'terminal.interrupt'
      );
      // Old binding no longer matches
      expect(store.matchAction(keydown('c', { ctrlKey: true }))).toBeNull();
    });

    it('marks binding as custom after change', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: false,
        altKey: true,
        key: 'i',
      });
      expect(store.isCustom('terminal.interrupt')).toBe(true);
    });

    it('persists to localStorage', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      expect(storage.has('godly-custom-keybindings')).toBe(true);

      // A new store picks up the override
      const store2 = new KeybindingStore();
      expect(chordToString(store2.getBinding('terminal.interrupt'))).toBe('Ctrl+Shift+x');
    });
  });

  describe('resetBinding', () => {
    it('reverts a single action to default', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      store.resetBinding('terminal.interrupt');
      expect(store.isCustom('terminal.interrupt')).toBe(false);
      expect(store.matchAction(keydown('c', { ctrlKey: true }))).toBe('terminal.interrupt');
    });
  });

  describe('resetAll', () => {
    it('reverts every action to defaults (verifies chord values)', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      store.setBinding('tabs.newTerminal', {
        ctrlKey: true,
        shiftKey: false,
        altKey: true,
        key: 'n',
      });
      store.resetAll();

      for (const def of DEFAULT_SHORTCUTS) {
        expect(store.isCustom(def.id)).toBe(false);
        expect(store.getBinding(def.id)).toEqual(def.defaultChord);
      }
      // Verify chord-index is also rebuilt: Ctrl+C should match again
      expect(store.matchAction(keydown('c', { ctrlKey: true }))).toBe('terminal.interrupt');
    });

    it('clears localStorage when all defaults', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      store.resetAll();
      expect(storage.has('godly-custom-keybindings')).toBe(false);
    });
  });

  describe('isAppShortcut / isTerminalControlKey', () => {
    it('classifies Ctrl+T as an app shortcut', () => {
      const store = new KeybindingStore();
      expect(store.isAppShortcut(keydown('t', { ctrlKey: true }))).toBe(true);
      expect(store.isTerminalControlKey(keydown('t', { ctrlKey: true }))).toBe(false);
    });

    it('classifies Ctrl+C as a terminal control key', () => {
      const store = new KeybindingStore();
      expect(store.isTerminalControlKey(keydown('c', { ctrlKey: true }))).toBe(true);
      expect(store.isAppShortcut(keydown('c', { ctrlKey: true }))).toBe(false);
    });

    it('type follows the action, not the binding', () => {
      // Bug scenario: user rebinds SIGINT to Ctrl+Shift+X — it should still
      // be classified as terminal-control so it gets preventDefault().
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      expect(store.isTerminalControlKey(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe(
        true
      );
      expect(store.isAppShortcut(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe(false);
    });

    it('returns false for keyup events', () => {
      const store = new KeybindingStore();
      expect(
        store.isAppShortcut({ key: 't', ctrlKey: true, shiftKey: false, altKey: false, type: 'keyup' })
      ).toBe(false);
      expect(
        store.isTerminalControlKey({
          key: 'c',
          ctrlKey: true,
          shiftKey: false,
          altKey: false,
          type: 'keyup',
        })
      ).toBe(false);
    });
  });

  describe('findConflict', () => {
    it('detects a conflict with an existing binding', () => {
      const store = new KeybindingStore();
      // Ctrl+C is already bound to terminal.interrupt
      const conflict = store.findConflict(
        { ctrlKey: true, shiftKey: false, altKey: false, key: 'c' },
        'tabs.newTerminal'
      );
      expect(conflict).toBe('terminal.interrupt');
    });

    it('does not conflict with itself (excludeAction)', () => {
      const store = new KeybindingStore();
      const conflict = store.findConflict(
        { ctrlKey: true, shiftKey: false, altKey: false, key: 'c' },
        'terminal.interrupt'
      );
      expect(conflict).toBeNull();
    });

    it('returns null for an unbound chord', () => {
      const store = new KeybindingStore();
      const conflict = store.findConflict({
        ctrlKey: true,
        shiftKey: true,
        altKey: true,
        key: 'x',
      });
      expect(conflict).toBeNull();
    });
  });

  describe('subscribe', () => {
    it('notifies on setBinding', () => {
      const store = new KeybindingStore();
      const fn = vi.fn();
      store.subscribe(fn);
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('notifies on resetBinding', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      const fn = vi.fn();
      store.subscribe(fn);
      store.resetBinding('terminal.interrupt');
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('notifies on resetAll', () => {
      const store = new KeybindingStore();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      const fn = vi.fn();
      store.subscribe(fn);
      store.resetAll();
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('unsubscribe stops notifications', () => {
      const store = new KeybindingStore();
      const fn = vi.fn();
      const unsub = store.subscribe(fn);
      unsub();
      store.setBinding('terminal.interrupt', {
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'x',
      });
      expect(fn).not.toHaveBeenCalled();
    });
  });

  describe('CapsLock edge case', () => {
    // Bug: With CapsLock on, browser sends uppercase key without shiftKey
    it('matches Ctrl+C when CapsLock sends uppercase C without shift', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('C', { ctrlKey: true }))).toBe('terminal.interrupt');
    });

    it('matches Ctrl+V when CapsLock sends uppercase V without shift', () => {
      const store = new KeybindingStore();
      expect(store.matchAction(keydown('V', { ctrlKey: true }))).toBe('terminal.literalNext');
    });
  });

  describe('persistence round-trip', () => {
    it('custom bindings survive a new store instance', () => {
      const store1 = new KeybindingStore();
      store1.setBinding('tabs.newTerminal', {
        ctrlKey: true,
        shiftKey: false,
        altKey: true,
        key: 'n',
      });
      store1.setBinding('clipboard.copy', {
        ctrlKey: true,
        shiftKey: false,
        altKey: false,
        key: 'y',
      });

      const store2 = new KeybindingStore();
      expect(chordToString(store2.getBinding('tabs.newTerminal'))).toBe('Ctrl+Alt+n');
      expect(chordToString(store2.getBinding('clipboard.copy'))).toBe('Ctrl+y');
      // Unmodified bindings stay default
      expect(store2.isCustom('tabs.closeTerminal')).toBe(false);
    });

    it('handles corrupt localStorage gracefully', () => {
      storage.set('godly-custom-keybindings', '{{not json');
      // Should not throw
      const store = new KeybindingStore();
      const interruptDef = DEFAULT_SHORTCUTS.find((d) => d.id === 'terminal.interrupt')!;
      expect(store.getBinding('terminal.interrupt')).toEqual(interruptDef.defaultChord);
    });
  });
});

describe('helper functions', () => {
  describe('chordToString', () => {
    it('produces correct string for Ctrl+Shift+C', () => {
      expect(
        chordToString({ ctrlKey: true, shiftKey: true, altKey: false, key: 'c' })
      ).toBe('Ctrl+Shift+c');
    });

    it('produces correct string for Ctrl+Tab', () => {
      expect(
        chordToString({ ctrlKey: true, shiftKey: false, altKey: false, key: 'tab' })
      ).toBe('Ctrl+tab');
    });
  });

  describe('eventToChord', () => {
    it('normalises key to lowercase and preserves all modifier flags', () => {
      const chord = eventToChord({
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'C',
      });
      expect(chord).toEqual({
        ctrlKey: true,
        shiftKey: true,
        altKey: false,
        key: 'c',
      });
    });
  });

  describe('formatChord', () => {
    it('produces human-readable Ctrl+Shift+C', () => {
      expect(
        formatChord({ ctrlKey: true, shiftKey: true, altKey: false, key: 'c' })
      ).toBe('Ctrl+Shift+C');
    });

    it('capitalises Tab correctly', () => {
      expect(
        formatChord({ ctrlKey: true, shiftKey: false, altKey: false, key: 'tab' })
      ).toBe('Ctrl+Tab');
    });

    it('includes Alt when set', () => {
      expect(
        formatChord({ ctrlKey: true, shiftKey: false, altKey: true, key: 'n' })
      ).toBe('Ctrl+Alt+N');
    });
  });
});
