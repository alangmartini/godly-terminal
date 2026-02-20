import { describe, it, expect, beforeEach } from 'vitest';
import {
  KeybindingStore,
} from '../state/keybinding-store';

/**
 * Bug #181: Home/End keys don't move cursor to beginning/end of input line.
 *
 * Plain Home and End were bound to scroll.toTop / scroll.toBottom app shortcuts,
 * so they were intercepted in handleKeyEvent() before reaching keyToTerminalData().
 * The escape sequences \x1b[H (Home) and \x1b[F (End) were never sent to the PTY.
 *
 * Fix: Rebind scroll.toTop/toBottom to Ctrl+Shift+Home/End (matches Windows Terminal).
 * Plain Home/End now pass through to the PTY.
 */

// ── Mirror of TerminalPane.keyToTerminalData() ──────────────────────────
// Exact copy of the private method so we can unit-test it without DOM.
function keyToTerminalData(event: {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  code?: string;
}): string | null {
  if (event.ctrlKey && !event.altKey && !event.shiftKey) {
    const key = event.key.toLowerCase();
    if (key.length === 1 && key >= 'a' && key <= 'z') {
      return String.fromCharCode(key.charCodeAt(0) - 96);
    }
    if (key === '[') return '\x1b';
    if (key === '\\') return '\x1c';
    if (key === ']') return '\x1d';
    if (key === ' ' || event.code === 'Space') return '\x00';
  }
  if (event.ctrlKey && event.altKey && !event.shiftKey) {
    const key = event.key.toLowerCase();
    if (key.length === 1 && key >= 'a' && key <= 'z') {
      return '\x1b' + String.fromCharCode(key.charCodeAt(0) - 96);
    }
  }
  if (event.altKey && !event.ctrlKey && event.key.length === 1) {
    return '\x1b' + event.key;
  }
  const mod = 1
    + (event.shiftKey ? 1 : 0)
    + (event.altKey ? 2 : 0)
    + (event.ctrlKey ? 4 : 0);
  switch (event.key) {
    case 'Enter': return '\r';
    case 'Backspace': return '\x7f';
    case 'Tab': return '\t';
    case 'Escape': return '\x1b';
    case 'ArrowUp':    return mod > 1 ? `\x1b[1;${mod}A` : '\x1b[A';
    case 'ArrowDown':  return mod > 1 ? `\x1b[1;${mod}B` : '\x1b[B';
    case 'ArrowRight': return mod > 1 ? `\x1b[1;${mod}C` : '\x1b[C';
    case 'ArrowLeft':  return mod > 1 ? `\x1b[1;${mod}D` : '\x1b[D';
    case 'Home': return mod > 1 ? `\x1b[1;${mod}H` : '\x1b[H';
    case 'End':  return mod > 1 ? `\x1b[1;${mod}F` : '\x1b[F';
    case 'Delete':   return mod > 1 ? `\x1b[3;${mod}~` : '\x1b[3~';
    case 'PageUp':   return mod > 1 ? `\x1b[5;${mod}~` : '\x1b[5~';
    case 'PageDown': return mod > 1 ? `\x1b[6;${mod}~` : '\x1b[6~';
    case 'Insert':   return mod > 1 ? `\x1b[2;${mod}~` : '\x1b[2~';
    case 'F1': return mod > 1 ? `\x1b[1;${mod}P` : '\x1bOP';
    case 'F2': return mod > 1 ? `\x1b[1;${mod}Q` : '\x1bOQ';
    case 'F3': return mod > 1 ? `\x1b[1;${mod}R` : '\x1bOR';
    case 'F4': return mod > 1 ? `\x1b[1;${mod}S` : '\x1bOS';
    case 'F5':  return mod > 1 ? `\x1b[15;${mod}~` : '\x1b[15~';
    case 'F6':  return mod > 1 ? `\x1b[17;${mod}~` : '\x1b[17~';
    case 'F7':  return mod > 1 ? `\x1b[18;${mod}~` : '\x1b[18~';
    case 'F8':  return mod > 1 ? `\x1b[19;${mod}~` : '\x1b[19~';
    case 'F9':  return mod > 1 ? `\x1b[20;${mod}~` : '\x1b[20~';
    case 'F10': return mod > 1 ? `\x1b[21;${mod}~` : '\x1b[21~';
    case 'F11': return mod > 1 ? `\x1b[23;${mod}~` : '\x1b[23~';
    case 'F12': return mod > 1 ? `\x1b[24;${mod}~` : '\x1b[24~';
  }
  return null;
}

function makeEvent(key: string, opts: {
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  code?: string;
} = {}) {
  return {
    key,
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    code: opts.code,
    type: 'keydown' as const,
  };
}

/**
 * Simulates the handleKeyEvent decision flow from TerminalPane.ts.
 *
 * Returns:
 *   - { action: 'scroll.toTop' | ... } if an app shortcut intercepts the key
 *   - { pty: string } if keyToTerminalData produces data to send to PTY
 *   - { passthrough: true } if no action and no terminal data
 */
function simulateHandleKeyEvent(
  store: KeybindingStore,
  event: ReturnType<typeof makeEvent>,
  onAlternateScreen = false,
): { action: string } | { pty: string } | { passthrough: true } {
  const action = store.matchAction(event);

  // Scroll shortcuts are handled locally — NOT sent to PTY
  // (unless on alternate screen)
  if (!onAlternateScreen) {
    if (action === 'scroll.pageUp') return { action: 'scroll.pageUp' };
    if (action === 'scroll.pageDown') return { action: 'scroll.pageDown' };
    if (action === 'scroll.toTop') return { action: 'scroll.toTop' };
    if (action === 'scroll.toBottom') return { action: 'scroll.toBottom' };
  }

  // Other app shortcuts bubble — not sent to PTY either
  if (action && store.isAppShortcut(event)) {
    return { action };
  }

  // Convert to terminal data
  const data = keyToTerminalData(event);
  if (data) {
    return { pty: data };
  }

  return { passthrough: true };
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #181: Home/End keys must reach PTY', () => {
  let store: KeybindingStore;

  beforeEach(() => {
    store = new KeybindingStore();
  });

  describe('Plain Home/End are not bound to app shortcuts', () => {
    it('plain Home is not bound to any action', () => {
      // Bug #181: was bound to scroll.toTop, preventing \x1b[H from reaching PTY
      const action = store.matchAction(makeEvent('Home'));
      expect(action).toBeNull();
    });

    it('plain End is not bound to any action', () => {
      // Bug #181: was bound to scroll.toBottom, preventing \x1b[F from reaching PTY
      const action = store.matchAction(makeEvent('End'));
      expect(action).toBeNull();
    });
  });

  describe('Plain Home/End send escape sequences to PTY', () => {
    it('Home sends \\x1b[H to the PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('Home'));
      expect(result).toEqual({ pty: '\x1b[H' });
    });

    it('End sends \\x1b[F to the PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('End'));
      expect(result).toEqual({ pty: '\x1b[F' });
    });

    it('Home on alternate screen sends \\x1b[H to PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('Home'), true);
      expect(result).toEqual({ pty: '\x1b[H' });
    });

    it('End on alternate screen sends \\x1b[F to PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('End'), true);
      expect(result).toEqual({ pty: '\x1b[F' });
    });
  });

  describe('Ctrl+Home/End still reach PTY (unmodified by fix)', () => {
    it('Ctrl+Home sends \\x1b[1;5H to PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('Home', { ctrlKey: true }));
      expect(result).toEqual({ pty: '\x1b[1;5H' });
    });

    it('Ctrl+End sends \\x1b[1;5F to PTY', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('End', { ctrlKey: true }));
      expect(result).toEqual({ pty: '\x1b[1;5F' });
    });
  });

  describe('Scroll to top/bottom now uses Ctrl+Shift+Home/End', () => {
    it('Ctrl+Shift+Home triggers scroll.toTop', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('Home', { ctrlKey: true, shiftKey: true }));
      expect(result).toEqual({ action: 'scroll.toTop' });
    });

    it('Ctrl+Shift+End triggers scroll.toBottom', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('End', { ctrlKey: true, shiftKey: true }));
      expect(result).toEqual({ action: 'scroll.toBottom' });
    });
  });

  describe('PageUp/PageDown scroll bindings unchanged', () => {
    it('PageUp triggers scroll.pageUp', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('PageUp'));
      expect(result).toEqual({ action: 'scroll.pageUp' });
    });

    it('PageDown triggers scroll.pageDown', () => {
      const result = simulateHandleKeyEvent(store, makeEvent('PageDown'));
      expect(result).toEqual({ action: 'scroll.pageDown' });
    });
  });
});
