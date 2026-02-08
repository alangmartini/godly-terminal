import { describe, it, expect, afterEach } from 'vitest';
import { isAppShortcut, isTerminalControlKey } from './keyboard';
import { keybindingStore } from '../state/keybinding-store';

function keydown(key: string, opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {}) {
  return {
    key,
    type: 'keydown',
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
  };
}

function keyup(key: string, opts: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {}) {
  return {
    key,
    type: 'keyup',
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
  };
}

describe('isAppShortcut', () => {
  // Bug: keyboard shortcuts stopped working when text was selected in the terminal
  // because xterm.js consumed the keydown event as terminal input (e.g. Ctrl+T = ASCII DC4).

  it('returns true for Ctrl+T (new terminal)', () => {
    expect(isAppShortcut(keydown('t', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+W (close terminal)', () => {
    expect(isAppShortcut(keydown('w', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Tab (next tab)', () => {
    expect(isAppShortcut(keydown('Tab', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+Tab (previous tab)', () => {
    expect(isAppShortcut(keydown('Tab', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+S (manual save)', () => {
    expect(isAppShortcut(keydown('S', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+L (manual load)', () => {
    expect(isAppShortcut(keydown('L', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+C (copy selection)', () => {
    expect(isAppShortcut(keydown('C', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+V (paste from clipboard)', () => {
    expect(isAppShortcut(keydown('V', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns false for Ctrl+C (terminal interrupt — must pass through)', () => {
    expect(isAppShortcut(keydown('c', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Ctrl+D (terminal EOF)', () => {
    expect(isAppShortcut(keydown('d', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Ctrl+L (terminal clear)', () => {
    expect(isAppShortcut(keydown('l', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for plain character keys', () => {
    expect(isAppShortcut(keydown('a'))).toBe(false);
    expect(isAppShortcut(keydown('t'))).toBe(false);
  });

  it('returns false for keyup events (only keydown matters)', () => {
    expect(isAppShortcut(keyup('t', { ctrlKey: true }))).toBe(false);
    expect(isAppShortcut(keyup('w', { ctrlKey: true }))).toBe(false);
    expect(isAppShortcut(keyup('Tab', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Shift+key without Ctrl', () => {
    expect(isAppShortcut(keydown('S', { shiftKey: true }))).toBe(false);
    expect(isAppShortcut(keydown('T', { shiftKey: true }))).toBe(false);
  });
});

describe('isTerminalControlKey', () => {
  // Bug: WebView2 intercepts Ctrl+C (copy) and Ctrl+Z (undo) at the browser
  // level, preventing xterm.js from sending SIGINT/SIGTSTP to the PTY.
  // These keys need event.preventDefault() so the browser doesn't consume them.

  it('returns true for Ctrl+C (SIGINT — browser would intercept as copy)', () => {
    expect(isTerminalControlKey(keydown('c', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Z (SIGTSTP — browser would intercept as undo)', () => {
    expect(isTerminalControlKey(keydown('z', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+V (literal next — browser would intercept as paste)', () => {
    expect(isTerminalControlKey(keydown('v', { ctrlKey: true }))).toBe(true);
  });

  it('returns false for Ctrl+Shift variants (those are app clipboard shortcuts)', () => {
    expect(isTerminalControlKey(keydown('C', { ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(isTerminalControlKey(keydown('V', { ctrlKey: true, shiftKey: true }))).toBe(false);
  });

  it('returns false for other Ctrl keys (no browser conflict)', () => {
    expect(isTerminalControlKey(keydown('d', { ctrlKey: true }))).toBe(false);
    expect(isTerminalControlKey(keydown('l', { ctrlKey: true }))).toBe(false);
    expect(isTerminalControlKey(keydown('a', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for keyup events', () => {
    expect(isTerminalControlKey(keyup('c', { ctrlKey: true }))).toBe(false);
    expect(isTerminalControlKey(keyup('z', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for keys without Ctrl', () => {
    expect(isTerminalControlKey(keydown('c'))).toBe(false);
    expect(isTerminalControlKey(keydown('z'))).toBe(false);
  });

  it('handles uppercase keys from CapsLock (no shiftKey set)', () => {
    expect(isTerminalControlKey(keydown('C', { ctrlKey: true }))).toBe(true);
    expect(isTerminalControlKey(keydown('Z', { ctrlKey: true }))).toBe(true);
    expect(isTerminalControlKey(keydown('V', { ctrlKey: true }))).toBe(true);
  });
});

describe('custom keybinding integration', () => {
  // Verify that keyboard.ts delegates to the keybinding store for custom bindings

  afterEach(() => {
    keybindingStore.resetAll();
  });

  it('isAppShortcut respects custom bindings from the store', () => {
    // Rebind tabs.newTerminal from Ctrl+T to Ctrl+Alt+N
    keybindingStore.setBinding('tabs.newTerminal', {
      ctrlKey: true,
      shiftKey: false,
      altKey: true,
      key: 'n',
    });

    // Old binding should no longer be an app shortcut
    expect(isAppShortcut(keydown('t', { ctrlKey: true }))).toBe(false);
    // New binding should be an app shortcut
    expect(isAppShortcut(keydown('n', { ctrlKey: true, altKey: true }))).toBe(true);
  });

  it('isTerminalControlKey respects custom bindings from the store', () => {
    // Rebind terminal.interrupt from Ctrl+C to Ctrl+Shift+X
    keybindingStore.setBinding('terminal.interrupt', {
      ctrlKey: true,
      shiftKey: true,
      altKey: false,
      key: 'x',
    });

    // Old binding should no longer be a terminal control key
    expect(isTerminalControlKey(keydown('c', { ctrlKey: true }))).toBe(false);
    // New binding should be a terminal control key
    expect(isTerminalControlKey(keydown('X', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('Ctrl+, is always an app shortcut regardless of bindings', () => {
    expect(isAppShortcut(keydown(',', { ctrlKey: true }))).toBe(true);
  });
});
