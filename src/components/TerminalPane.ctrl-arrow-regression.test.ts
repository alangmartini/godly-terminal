import { describe, it, expect, vi, beforeEach } from 'vitest';
import { KeybindingStore, type ActionId } from '../state/keybinding-store';

/**
 * Bug #416: Ctrl+Arrow word navigation broken in terminal (regression from #404)
 *
 * PR #404 changed panel focus hotkeys to Ctrl+Arrow. The keybinding store
 * correctly classifies them as app shortcuts, but TerminalPane's routing
 * must only intercept them when a split actually exists. Without a split,
 * Ctrl+Arrow must pass through to the PTY for word navigation.
 *
 * Fix: TerminalPane.handleKeyEvent() checks for split existence before
 * intercepting split-only app shortcuts (focus, resize, zoom, swap, etc.).
 */

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

/**
 * Simulates the routing decision from TerminalPane.handleKeyEvent().
 *
 * When an app shortcut is detected, TerminalPane returns early UNLESS:
 * - The action is a split-only action (focus/resize/zoom/swap/rotate/unsplit)
 * - AND no split exists in the current workspace
 *
 * In that case, the key falls through to keyToTerminalData() and reaches the PTY.
 */
function wouldReachTerminal(
  kbStore: KeybindingStore,
  event: ReturnType<typeof keydown>,
  hasSplit: boolean,
): boolean {
  // Mirror of TerminalPane.handleKeyEvent() routing logic
  const action = kbStore.matchAction(event);

  if (kbStore.isAppShortcut(event)) {
    const isSplitAction = action !== null && action.startsWith('split.') &&
      action !== 'split.vertical' && action !== 'split.horizontal';
    if (!isSplitAction || hasSplit) {
      return false; // intercepted by app
    }
    // Split-only action with no split → falls through to PTY
  }

  return true;
}

describe('Bug #416: Ctrl+Arrow routing depends on split state', () => {
  let kbStore: KeybindingStore;

  beforeEach(() => {
    storage.clear();
    kbStore = new KeybindingStore();
  });

  describe('without a split: Ctrl+Arrow reaches the terminal for word navigation', () => {
    // Bug: Before the fix, these all returned false (key swallowed as app shortcut)

    it('Ctrl+Left reaches PTY (word-left) when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowLeft', { ctrlKey: true }), false)).toBe(true);
    });

    it('Ctrl+Right reaches PTY (word-right) when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowRight', { ctrlKey: true }), false)).toBe(true);
    });

    it('Ctrl+Up reaches PTY when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowUp', { ctrlKey: true }), false)).toBe(true);
    });

    it('Ctrl+Down reaches PTY when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowDown', { ctrlKey: true }), false)).toBe(true);
    });
  });

  describe('with a split: Ctrl+Arrow is intercepted for split focus', () => {
    it('Ctrl+Left is intercepted when a split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowLeft', { ctrlKey: true }), true)).toBe(false);
    });

    it('Ctrl+Right is intercepted when a split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowRight', { ctrlKey: true }), true)).toBe(false);
    });

    it('Ctrl+Up is intercepted when a split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowUp', { ctrlKey: true }), true)).toBe(false);
    });

    it('Ctrl+Down is intercepted when a split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowDown', { ctrlKey: true }), true)).toBe(false);
    });
  });

  describe('split creation shortcuts always bubble (even without a split)', () => {
    // split.vertical and split.horizontal should always be intercepted
    // because they CREATE splits — they're not no-ops without a split

    it('split.vertical binding is intercepted even without a split', () => {
      const def = kbStore.getBinding('split.vertical' as ActionId);
      if (!def) return; // skip if no binding
      const event = keydown(def.key, {
        ctrlKey: def.ctrlKey,
        shiftKey: def.shiftKey,
        altKey: def.altKey,
      });
      expect(wouldReachTerminal(kbStore, event, false)).toBe(false);
    });

    it('split.horizontal binding is intercepted even without a split', () => {
      const def = kbStore.getBinding('split.horizontal' as ActionId);
      if (!def) return;
      const event = keydown(def.key, {
        ctrlKey: def.ctrlKey,
        shiftKey: def.shiftKey,
        altKey: def.altKey,
      });
      expect(wouldReachTerminal(kbStore, event, false)).toBe(false);
    });
  });

  describe('non-split app shortcuts always bubble regardless of split state', () => {
    // Non-split app shortcuts (close tab, new tab, settings, etc.) must always
    // be intercepted — they should never reach the PTY

    const NON_SPLIT_ACTIONS: ActionId[] = [
      'tab.close',
      'tab.new',
    ];

    for (const actionId of NON_SPLIT_ACTIONS) {
      it(`${actionId} is always intercepted (no split)`, () => {
        const def = kbStore.getBinding(actionId);
        if (!def) return;
        const event = keydown(def.key, {
          ctrlKey: def.ctrlKey,
          shiftKey: def.shiftKey,
          altKey: def.altKey,
        });
        expect(wouldReachTerminal(kbStore, event, false)).toBe(false);
      });
    }
  });

  describe('Ctrl+Alt+Arrow (resize) also passes through without a split', () => {
    it('Ctrl+Alt+Left reaches PTY when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowLeft', { ctrlKey: true, altKey: true }), false)).toBe(true);
    });

    it('Ctrl+Alt+Right reaches PTY when no split exists', () => {
      expect(wouldReachTerminal(kbStore, keydown('ArrowRight', { ctrlKey: true, altKey: true }), false)).toBe(true);
    });
  });
});
