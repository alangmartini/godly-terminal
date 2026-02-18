import { describe, it, expect, afterEach } from 'vitest';
import { keybindingStore } from '../state/keybinding-store';
import { isAppShortcut, isTerminalControlKey } from './keyboard';

/**
 * Tests for the custom key event handler logic in TerminalPane.ts.
 *
 * These tests reproduce three keyboard handling bugs:
 *
 * Bug 1: Ctrl+Shift+V pastes twice — the handler reads clipboard and writes to
 *   terminal, but never calls event.preventDefault(), so the browser also pastes.
 *
 * Bug 2: Ctrl+V pastes from clipboard — it should be sent to the PTY as a
 *   terminal "literal next" control character (\x16), not trigger a browser paste.
 *
 * Bug 3: Ctrl+C doesn't interrupt running processes — the interrupt signal is not
 *   reaching the PTY because WebView2 intercepts the key for clipboard copy.
 */

// ── Helpers ──────────────────────────────────────────────────────────────

function createMockKeyboardEvent(
  key: string,
  opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; type?: string } = {}
): KeyboardEvent & { preventDefaultCalled: boolean } {
  let preventDefaultCalled = false;
  const event = {
    key,
    type: opts.type ?? 'keydown',
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    preventDefault: () => {
      preventDefaultCalled = true;
    },
    get preventDefaultCalled() {
      return preventDefaultCalled;
    },
  };
  return event as any;
}

/**
 * Mirrors keyToTerminalData from TerminalPane.ts.
 * Returns non-null when the keydown handler would send data to the PTY.
 */
function keyToTerminalData(event: { key: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean }): string | null {
  if (event.ctrlKey && !event.altKey && !event.shiftKey) {
    const key = event.key.toLowerCase();
    if (key.length === 1 && key >= 'a' && key <= 'z') {
      return String.fromCharCode(key.charCodeAt(0) - 96);
    }
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
  // Special keys, function keys, etc. omitted — not needed for these tests
  return null;
}

/**
 * Reproduces the custom key event handler logic from TerminalPane.ts.
 * This mirrors the exact decision logic so we can test it without a full
 * Canvas2D renderer + Tauri environment. Keep in sync with the source.
 *
 * Returns:
 * - handlerReturn: true = event will be sent to PTY, false = consumed by app
 * - preventDefaultCalled: whether event.preventDefault() was called
 * - pasteTriggered: whether the handler initiated a clipboard paste
 */
function simulateCustomKeyHandler(event: ReturnType<typeof createMockKeyboardEvent>): {
  handlerReturn: boolean;
  preventDefaultCalled: boolean;
  pasteTriggered: boolean;
} {
  let pasteTriggered = false;

  // ── Mirrors TerminalPane.ts:86-127 ────────────────────────────────
  const action = keybindingStore.matchAction(event);

  if (action === 'clipboard.copy') {
    event.preventDefault();
    return {
      handlerReturn: false,
      preventDefaultCalled: event.preventDefaultCalled,
      pasteTriggered: false,
    };
  }

  if (action === 'clipboard.copyClean') {
    event.preventDefault();
    return {
      handlerReturn: false,
      preventDefaultCalled: event.preventDefaultCalled,
      pasteTriggered: false,
    };
  }

  if (action === 'clipboard.paste') {
    event.preventDefault();
    pasteTriggered = true;
    return {
      handlerReturn: false,
      preventDefaultCalled: event.preventDefaultCalled,
      pasteTriggered: true,
    };
  }

  if (isAppShortcut(event)) {
    return {
      handlerReturn: false,
      preventDefaultCalled: event.preventDefaultCalled,
      pasteTriggered: false,
    };
  }

  if (isTerminalControlKey(event)) {
    event.preventDefault();
  } else if (event.type === 'keyup' && isTerminalControlKey({
    ctrlKey: event.ctrlKey,
    shiftKey: event.shiftKey,
    altKey: event.altKey,
    key: event.key,
    type: 'keydown',
  })) {
    event.preventDefault();
  }

  // Mirror of keyToTerminalData: if it returns data, preventDefault is called.
  // This mirrors TerminalPane.ts lines 311-318. Only runs for keydown events
  // (handleKeyEvent is attached to the keydown listener only).
  if (event.type === 'keydown') {
    const data = keyToTerminalData(event);
    if (data) {
      event.preventDefault();
    }
  }

  return {
    handlerReturn: true,
    preventDefaultCalled: event.preventDefaultCalled,
    pasteTriggered,
  };
  // ── End of mirror ─────────────────────────────────────────────────
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('TerminalPane custom key event handler bugs', () => {
  afterEach(() => {
    keybindingStore.resetAll();
  });

  describe('Bug 1: Ctrl+Shift+V pastes twice', () => {
    // Bug: When Ctrl+Shift+V is pressed, the custom handler reads the clipboard
    // and writes the content to the terminal (one paste), then returns false
    // WITHOUT calling event.preventDefault(). The browser default paste action
    // fires a second time, causing the text to appear twice.

    it('must call preventDefault when handling clipboard.paste to prevent double paste', () => {
      const event = createMockKeyboardEvent('V', {
        ctrlKey: true,
        shiftKey: true,
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.pasteTriggered).toBe(true);
      expect(result.handlerReturn).toBe(false);
      expect(result.preventDefaultCalled).toBe(true);
    });
  });

  describe('Bug 2: Ctrl+V pastes from clipboard instead of sending literal next', () => {
    // Bug: Ctrl+V should map to terminal.literalNext (type: terminal-control),
    // sending \x16 to the PTY — not trigger a browser paste.

    it('Ctrl+V maps to terminal.literalNext, not clipboard.paste', () => {
      const event = createMockKeyboardEvent('v', { ctrlKey: true });
      const action = keybindingStore.matchAction(event);
      expect(action).toBe('terminal.literalNext');
    });

    it('Ctrl+V must call preventDefault and return true to block browser paste', () => {
      const event = createMockKeyboardEvent('v', { ctrlKey: true });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.pasteTriggered).toBe(false);
      expect(result.preventDefaultCalled).toBe(true);
    });
  });

  describe('Bug 3: Ctrl+C does not interrupt running terminal processes', () => {
    // Bug: Ctrl+C is mapped to terminal.interrupt (type: terminal-control).
    // The handler must: 1) call preventDefault, 2) return true so the PTY receives \x03.

    it('Ctrl+C keydown must call preventDefault and return true', () => {
      const event = createMockKeyboardEvent('c', { ctrlKey: true });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.preventDefaultCalled).toBe(true);
      expect(result.pasteTriggered).toBe(false);
    });

    it('Ctrl+C with CapsLock (uppercase C, no shift) must still work', () => {
      const event = createMockKeyboardEvent('C', { ctrlKey: true });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.preventDefaultCalled).toBe(true);
    });

    it('Ctrl+C keyup must also call preventDefault to prevent WebView2 copy', () => {
      // Bug: The canvas receives both keydown AND keyup events.
      // On keyup, matchAction returns null (only matches keydown), so the handler
      // falls through without calling preventDefault(). WebView2 may intercept the
      // keyup event and trigger a clipboard copy, preventing SIGINT from working.
      const event = createMockKeyboardEvent('c', {
        ctrlKey: true,
        type: 'keyup',
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.preventDefaultCalled).toBe(true);
    });
  });

  describe('clipboard.copyClean (Ctrl+Shift+Alt+C)', () => {
    it('must call preventDefault and return false', () => {
      const event = createMockKeyboardEvent('C', {
        ctrlKey: true,
        shiftKey: true,
        altKey: true,
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(false);
      expect(result.preventDefaultCalled).toBe(true);
      expect(result.pasteTriggered).toBe(false);
    });

    it('does not conflict with clipboard.copy (Ctrl+Shift+C)', () => {
      const copyEvent = createMockKeyboardEvent('C', {
        ctrlKey: true,
        shiftKey: true,
      });
      const copyCleanEvent = createMockKeyboardEvent('C', {
        ctrlKey: true,
        shiftKey: true,
        altKey: true,
      });

      expect(keybindingStore.matchAction(copyEvent)).toBe('clipboard.copy');
      expect(keybindingStore.matchAction(copyCleanEvent)).toBe('clipboard.copyClean');
    });
  });

  describe('Event flow integrity', () => {
    it('clipboard.copy (Ctrl+Shift+C) must call preventDefault and return false', () => {
      const event = createMockKeyboardEvent('C', {
        ctrlKey: true,
        shiftKey: true,
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(false);
      expect(result.preventDefaultCalled).toBe(true);
    });

    it('app shortcuts (Ctrl+T) return false without calling preventDefault', () => {
      const event = createMockKeyboardEvent('t', { ctrlKey: true });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(false);
      expect(result.preventDefaultCalled).toBe(false);
    });

    it('unbound keys pass through without calling preventDefault', () => {
      const event = createMockKeyboardEvent('a', {});
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.preventDefaultCalled).toBe(false);
    });

    it('keyup for non-terminal-control keys does not call preventDefault', () => {
      const event = createMockKeyboardEvent('t', {
        ctrlKey: true,
        type: 'keyup',
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      expect(result.preventDefaultCalled).toBe(false);
    });

    it('Ctrl+Alt+C must not leak bare "c" to the textarea input event', () => {
      // Bug: Ctrl+Alt+C falls through all handlers in keyToTerminalData,
      // so event.preventDefault is never called and the textarea receives "c".
      // The handler must return true with preventDefault called.
      const event = createMockKeyboardEvent('c', {
        ctrlKey: true,
        altKey: true,
      });
      const result = simulateCustomKeyHandler(event);

      expect(result.handlerReturn).toBe(true);
      // Must preventDefault so the textarea doesn't fire an input event with "c"
      expect(result.preventDefaultCalled).toBe(true);
    });
  });
});
