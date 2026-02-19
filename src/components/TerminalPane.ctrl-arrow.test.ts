import { describe, it, expect } from 'vitest';

/**
 * Bug: Ctrl+Arrow keys don't navigate by word in terminal applications.
 *
 * When pressing Ctrl+Left or Ctrl+Right, the terminal should send modified
 * CSI sequences (\x1b[1;5D and \x1b[1;5C respectively) so that shells and
 * CLI tools (bash, zsh, Claude Code, etc.) interpret them as word-navigation.
 *
 * The current keyToTerminalData() sends the same unmodified sequences
 * (\x1b[D, \x1b[C) for both plain Arrow and Ctrl+Arrow, so the modifier
 * is silently dropped and the cursor moves one character instead of one word.
 *
 * Standard CSI modifier encoding (param 2 in CSI 1;{mod}{key}):
 *   2 = Shift, 3 = Alt, 5 = Ctrl, 6 = Ctrl+Shift, 7 = Ctrl+Alt, 8 = Ctrl+Shift+Alt
 */

// ── Mirror of TerminalPane.keyToTerminalData() ──────────────────────────
// Exact copy of the private method from TerminalPane.ts so we can unit-test
// it without a full Canvas2D/Tauri environment. Keep in sync with the source.
function keyToTerminalData(event: {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  code?: string;
}): string | null {
  // Control key combinations -> control characters
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

  // Ctrl+Alt combinations -> ESC + control character
  if (event.ctrlKey && event.altKey && !event.shiftKey) {
    const key = event.key.toLowerCase();
    if (key.length === 1 && key >= 'a' && key <= 'z') {
      return '\x1b' + String.fromCharCode(key.charCodeAt(0) - 96);
    }
  }

  // Alt combinations -> ESC + key
  if (event.altKey && !event.ctrlKey && event.key.length === 1) {
    return '\x1b' + event.key;
  }

  // CSI modifier parameter for special keys:
  // 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
  const mod = 1
    + (event.shiftKey ? 1 : 0)
    + (event.altKey ? 2 : 0)
    + (event.ctrlKey ? 4 : 0);

  // Special keys
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

// ── Helper ──────────────────────────────────────────────────────────────

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
  };
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug: Ctrl+Arrow word navigation sends wrong escape sequences', () => {

  describe('Ctrl+Arrow must send CSI 1;5 modified sequences', () => {
    // Bug: Ctrl+Left/Right sends \x1b[D / \x1b[C (same as plain arrows),
    // so shells can't distinguish them and cursor moves by character, not word.

    it('Ctrl+Left sends \\x1b[1;5D (word left)', () => {
      const result = keyToTerminalData(makeEvent('ArrowLeft', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5D');
    });

    it('Ctrl+Right sends \\x1b[1;5C (word right)', () => {
      const result = keyToTerminalData(makeEvent('ArrowRight', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5C');
    });

    it('Ctrl+Up sends \\x1b[1;5A', () => {
      const result = keyToTerminalData(makeEvent('ArrowUp', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5A');
    });

    it('Ctrl+Down sends \\x1b[1;5B', () => {
      const result = keyToTerminalData(makeEvent('ArrowDown', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5B');
    });
  });

  describe('Shift+Arrow must send CSI 1;2 modified sequences', () => {
    it('Shift+Left sends \\x1b[1;2D', () => {
      const result = keyToTerminalData(makeEvent('ArrowLeft', { shiftKey: true }));
      expect(result).toBe('\x1b[1;2D');
    });

    it('Shift+Right sends \\x1b[1;2C', () => {
      const result = keyToTerminalData(makeEvent('ArrowRight', { shiftKey: true }));
      expect(result).toBe('\x1b[1;2C');
    });
  });

  describe('Ctrl+Shift+Arrow must send CSI 1;6 modified sequences', () => {
    it('Ctrl+Shift+Left sends \\x1b[1;6D', () => {
      const result = keyToTerminalData(makeEvent('ArrowLeft', { ctrlKey: true, shiftKey: true }));
      expect(result).toBe('\x1b[1;6D');
    });

    it('Ctrl+Shift+Right sends \\x1b[1;6C', () => {
      const result = keyToTerminalData(makeEvent('ArrowRight', { ctrlKey: true, shiftKey: true }));
      expect(result).toBe('\x1b[1;6C');
    });
  });

  describe('Alt+Arrow must send CSI 1;3 modified sequences', () => {
    it('Alt+Left sends \\x1b[1;3D', () => {
      const result = keyToTerminalData(makeEvent('ArrowLeft', { altKey: true }));
      expect(result).toBe('\x1b[1;3D');
    });

    it('Alt+Right sends \\x1b[1;3C', () => {
      const result = keyToTerminalData(makeEvent('ArrowRight', { altKey: true }));
      expect(result).toBe('\x1b[1;3C');
    });
  });

  describe('Ctrl+Home/End must send modified sequences', () => {
    it('Ctrl+Home sends \\x1b[1;5H', () => {
      const result = keyToTerminalData(makeEvent('Home', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5H');
    });

    it('Ctrl+End sends \\x1b[1;5F', () => {
      const result = keyToTerminalData(makeEvent('End', { ctrlKey: true }));
      expect(result).toBe('\x1b[1;5F');
    });
  });

  describe('plain arrows still work unmodified', () => {
    it('ArrowLeft sends \\x1b[D', () => {
      const result = keyToTerminalData(makeEvent('ArrowLeft'));
      expect(result).toBe('\x1b[D');
    });

    it('ArrowRight sends \\x1b[C', () => {
      const result = keyToTerminalData(makeEvent('ArrowRight'));
      expect(result).toBe('\x1b[C');
    });

    it('ArrowUp sends \\x1b[A', () => {
      const result = keyToTerminalData(makeEvent('ArrowUp'));
      expect(result).toBe('\x1b[A');
    });

    it('ArrowDown sends \\x1b[B', () => {
      const result = keyToTerminalData(makeEvent('ArrowDown'));
      expect(result).toBe('\x1b[B');
    });
  });
});
