// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for dead key and IME composition support via hidden textarea.
 *
 * Bug: On Brazilian/Portuguese (ABNT2) keyboards, quote characters (' and ")
 * are dead keys. Pressing them fires event.key="Dead" (length 4), which falls
 * through all handlers in keyToTerminalData() and returns null — the character
 * never reaches the PTY.
 *
 * Root cause: Canvas elements don't participate in OS text composition. Dead
 * keys and IME sequences only produce composed characters on editable elements
 * (input, textarea, contenteditable).
 *
 * Fix: A hidden <textarea> captures keyboard input. Special keys (Enter,
 * arrows, Ctrl combos) are still handled in keydown with preventDefault().
 * Printable characters and dead-key compositions flow through the textarea's
 * input event, which correctly resolves dead keys and IME sequences.
 */

// ── Helpers ──────────────────────────────────────────────────────────────

/**
 * Mirrors the keyToTerminalData logic from TerminalPane.ts.
 * After the fix, printable characters are no longer handled here — they
 * flow through the textarea's input event instead.
 */
function keyToTerminalData(event: {
  key: string;
  code?: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
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

  switch (event.key) {
    case 'Enter': return '\r';
    case 'Backspace': return '\x7f';
    case 'Tab': return '\t';
    case 'Escape': return '\x1b';
    case 'Delete': return '\x1b[3~';
    case 'ArrowUp': return '\x1b[A';
    case 'ArrowDown': return '\x1b[B';
    case 'ArrowRight': return '\x1b[C';
    case 'ArrowLeft': return '\x1b[D';
    case 'Home': return '\x1b[H';
    case 'End': return '\x1b[F';
    case 'PageUp': return '\x1b[5~';
    case 'PageDown': return '\x1b[6~';
    case 'Insert': return '\x1b[2~';
    case 'F1': return '\x1bOP';
    case 'F2': return '\x1bOQ';
    case 'F3': return '\x1bOR';
    case 'F4': return '\x1bOS';
    case 'F5': return '\x1b[15~';
    case 'F6': return '\x1b[17~';
    case 'F7': return '\x1b[18~';
    case 'F8': return '\x1b[19~';
    case 'F9': return '\x1b[20~';
    case 'F10': return '\x1b[21~';
    case 'F11': return '\x1b[23~';
    case 'F12': return '\x1b[24~';
  }

  // Printable characters NOT handled here — textarea input event handles them
  return null;
}

/**
 * Simulates the hidden textarea input pipeline:
 * 1. keydown fires on textarea
 * 2. If keyToTerminalData returns non-null, preventDefault blocks the input event
 * 3. If keyToTerminalData returns null, the browser processes the key normally
 * 4. For printable keys, the textarea fires an input event with the composed text
 *
 * Returns what would be sent to the PTY.
 */
function simulateTextareaInput(
  keydownEvent: { key: string; code?: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean },
  composedText: string | null,
): { sentToPty: string | null; source: 'keydown' | 'input' } {
  const data = keyToTerminalData(keydownEvent);
  if (data) {
    // keydown handler sends data and calls preventDefault — textarea doesn't fire input
    return { sentToPty: data, source: 'keydown' };
  }
  // keydown didn't handle it — textarea's input event fires with composed text
  if (composedText) {
    return { sentToPty: composedText, source: 'input' };
  }
  return { sentToPty: null, source: 'input' };
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Dead key support (ABNT2 keyboard quote fix)', () => {
  describe('keyToTerminalData no longer handles printable characters', () => {
    // Bug: Previously, keyToTerminalData returned event.key for printable chars.
    // This prevented dead keys from working because the dead key's first press
    // (key="Dead") returned null, and the second press's composed character
    // never reached the terminal if it was also handled in keydown.

    it('returns null for regular printable characters (a, z, 1, @, etc.)', () => {
      const chars = ['a', 'z', 'A', 'Z', '1', '0', '@', '#', '!', '/', '.', ',', ';'];
      for (const ch of chars) {
        expect(keyToTerminalData({ key: ch, ctrlKey: false, altKey: false, shiftKey: false }))
          .toBe(null);
      }
    });

    it('returns null for dead key event (key="Dead")', () => {
      // ABNT2 keyboard: pressing ' (acute accent dead key)
      expect(keyToTerminalData({
        key: 'Dead', code: 'BracketRight',
        ctrlKey: false, altKey: false, shiftKey: false,
      })).toBe(null);
    });

    it('returns null for Shift+dead key (key="Dead", e.g. Shift+\' for ")', () => {
      // ABNT2 keyboard: pressing Shift+' (double quote dead key)
      expect(keyToTerminalData({
        key: 'Dead', code: 'BracketRight',
        ctrlKey: false, altKey: false, shiftKey: true,
      })).toBe(null);
    });

    it('returns null for composed accent character (e.g. á from dead key + a)', () => {
      // After dead key resolution, the browser may fire keydown with the composed char
      expect(keyToTerminalData({ key: 'á', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe(null);
    });
  });

  describe('Special keys still handled in keydown', () => {
    it('Enter sends \\r', () => {
      expect(keyToTerminalData({ key: 'Enter', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\r');
    });

    it('Backspace sends DEL', () => {
      expect(keyToTerminalData({ key: 'Backspace', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\x7f');
    });

    it('Tab sends \\t', () => {
      expect(keyToTerminalData({ key: 'Tab', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\t');
    });

    it('Escape sends ESC', () => {
      expect(keyToTerminalData({ key: 'Escape', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\x1b');
    });

    it('arrow keys send escape sequences', () => {
      expect(keyToTerminalData({ key: 'ArrowUp', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\x1b[A');
      expect(keyToTerminalData({ key: 'ArrowDown', ctrlKey: false, altKey: false, shiftKey: false }))
        .toBe('\x1b[B');
    });
  });

  describe('Ctrl and Alt combos still handled in keydown', () => {
    it('Ctrl+C sends ETX (interrupt)', () => {
      expect(keyToTerminalData({ key: 'c', ctrlKey: true, altKey: false, shiftKey: false }))
        .toBe('\x03');
    });

    it('Ctrl+D sends EOT', () => {
      expect(keyToTerminalData({ key: 'd', ctrlKey: true, altKey: false, shiftKey: false }))
        .toBe('\x04');
    });

    it('Alt+a sends ESC+a', () => {
      expect(keyToTerminalData({ key: 'a', ctrlKey: false, altKey: true, shiftKey: false }))
        .toBe('\x1ba');
    });
  });

  describe('Bug: Ctrl+Alt+letter sends raw character instead of ESC+control char', () => {
    // Bug: Pressing Ctrl+Alt+C types "c" because keyToTerminalData returns null
    // for Ctrl+Alt combos. The Ctrl branch requires !altKey and the Alt branch
    // requires !ctrlKey, so Ctrl+Alt falls through both. Since no handler calls
    // preventDefault(), the textarea input event fires with the bare character.
    //
    // Standard terminal behavior: Ctrl+Alt+letter → ESC + control character
    // e.g. Ctrl+Alt+C → \x1b\x03 (ESC + ETX)

    it('Ctrl+Alt+C sends ESC + ETX (\\x1b\\x03)', () => {
      expect(keyToTerminalData({ key: 'c', ctrlKey: true, altKey: true, shiftKey: false }))
        .toBe('\x1b\x03');
    });

    it('Ctrl+Alt+D sends ESC + EOT (\\x1b\\x04)', () => {
      expect(keyToTerminalData({ key: 'd', ctrlKey: true, altKey: true, shiftKey: false }))
        .toBe('\x1b\x04');
    });

    it('Ctrl+Alt+A sends ESC + SOH (\\x1b\\x01)', () => {
      expect(keyToTerminalData({ key: 'a', ctrlKey: true, altKey: true, shiftKey: false }))
        .toBe('\x1b\x01');
    });

    it('Ctrl+Alt+Z sends ESC + SUB (\\x1b\\x1a)', () => {
      expect(keyToTerminalData({ key: 'z', ctrlKey: true, altKey: true, shiftKey: false }))
        .toBe('\x1b\x1a');
    });

    it('Ctrl+Alt+letter is handled in keydown, not textarea input', () => {
      const result = simulateTextareaInput(
        { key: 'c', ctrlKey: true, altKey: true, shiftKey: false },
        null,
      );
      expect(result.sentToPty).toBe('\x1b\x03');
      expect(result.source).toBe('keydown');
    });
  });

  describe('Textarea input pipeline for dead keys', () => {
    it('dead key press followed by space produces quote character via input event', () => {
      // Step 1: Dead key press → keydown with key="Dead"
      const deadKeyResult = simulateTextareaInput(
        { key: 'Dead', code: 'BracketRight', ctrlKey: false, altKey: false, shiftKey: false },
        null,  // Dead key press alone produces no composed text
      );
      expect(deadKeyResult.sentToPty).toBe(null);

      // Step 2: Space press → keydown not handled, textarea fires input with "'"
      const spaceResult = simulateTextareaInput(
        { key: "'", ctrlKey: false, altKey: false, shiftKey: false },
        "'",  // Textarea receives the resolved dead key character
      );
      expect(spaceResult.sentToPty).toBe("'");
      expect(spaceResult.source).toBe('input');
    });

    it('Shift+dead key followed by space produces double quote via input event', () => {
      // Shift+' on ABNT2 = " (double quote dead key)
      const deadKeyResult = simulateTextareaInput(
        { key: 'Dead', code: 'BracketRight', ctrlKey: false, altKey: false, shiftKey: true },
        null,
      );
      expect(deadKeyResult.sentToPty).toBe(null);

      const spaceResult = simulateTextareaInput(
        { key: '"', ctrlKey: false, altKey: false, shiftKey: false },
        '"',
      );
      expect(spaceResult.sentToPty).toBe('"');
      expect(spaceResult.source).toBe('input');
    });

    it('dead key followed by vowel produces accented character via input event', () => {
      // ' + a = á
      const deadKeyResult = simulateTextareaInput(
        { key: 'Dead', code: 'BracketRight', ctrlKey: false, altKey: false, shiftKey: false },
        null,
      );
      expect(deadKeyResult.sentToPty).toBe(null);

      const vowelResult = simulateTextareaInput(
        { key: 'á', ctrlKey: false, altKey: false, shiftKey: false },
        'á',
      );
      expect(vowelResult.sentToPty).toBe('á');
      expect(vowelResult.source).toBe('input');
    });

    it('regular characters flow through input event', () => {
      const result = simulateTextareaInput(
        { key: 'a', ctrlKey: false, altKey: false, shiftKey: false },
        'a',
      );
      expect(result.sentToPty).toBe('a');
      expect(result.source).toBe('input');
    });

    it('special keys bypass input event and go through keydown', () => {
      const result = simulateTextareaInput(
        { key: 'Enter', ctrlKey: false, altKey: false, shiftKey: false },
        null,  // preventDefault blocks input event
      );
      expect(result.sentToPty).toBe('\r');
      expect(result.source).toBe('keydown');
    });

    it('Ctrl combos bypass input event and go through keydown', () => {
      const result = simulateTextareaInput(
        { key: 'c', ctrlKey: true, altKey: false, shiftKey: false },
        null,
      );
      expect(result.sentToPty).toBe('\x03');
      expect(result.source).toBe('keydown');
    });
  });

  describe('Hidden textarea DOM integration', () => {
    let textarea: HTMLTextAreaElement;
    let sentToTerminal: string[];

    beforeEach(() => {
      sentToTerminal = [];
      textarea = document.createElement('textarea');
      textarea.style.cssText = 'position:absolute;left:-9999px;opacity:0;';
      document.body.appendChild(textarea);

      // Simulates the input event handler from TerminalPane.ts
      textarea.addEventListener('input', () => {
        const text = textarea.value;
        if (text) {
          sentToTerminal.push(text);
          textarea.value = '';
        }
      });
    });

    afterEach(() => {
      textarea.remove();
    });

    it('textarea captures typed text via value + input event', () => {
      // Simulate what the browser does when a printable key is pressed on a textarea
      textarea.value = 'a';
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual(['a']);
      expect(textarea.value).toBe(''); // Cleared after send
    });

    it('textarea captures multi-char composed text (e.g. from dead key)', () => {
      // After dead key composition, the textarea receives the resolved character
      textarea.value = "'";
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual(["'"]);
    });

    it('textarea captures accented character from dead key + vowel', () => {
      textarea.value = 'á';
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual(['á']);
    });

    it('empty input event does not send to terminal', () => {
      textarea.value = '';
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual([]);
    });
  });

  describe('IME composition tracking', () => {
    let textarea: HTMLTextAreaElement;
    let sentToTerminal: string[];
    let isComposing: boolean;

    beforeEach(() => {
      sentToTerminal = [];
      isComposing = false;
      textarea = document.createElement('textarea');
      document.body.appendChild(textarea);

      textarea.addEventListener('compositionstart', () => {
        isComposing = true;
      });
      textarea.addEventListener('compositionend', () => {
        isComposing = false;
        const text = textarea.value;
        if (text) {
          sentToTerminal.push(text);
        }
        textarea.value = '';
      });
      textarea.addEventListener('input', () => {
        if (isComposing) return;
        const text = textarea.value;
        if (text) {
          sentToTerminal.push(text);
          textarea.value = '';
        }
      });
    });

    afterEach(() => {
      textarea.remove();
    });

    it('intermediate composition input is not sent to terminal', () => {
      textarea.dispatchEvent(new CompositionEvent('compositionstart'));

      // Intermediate IME text
      textarea.value = 'にほ';
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual([]); // Not sent yet
    });

    it('final composed text is sent on compositionend', () => {
      textarea.dispatchEvent(new CompositionEvent('compositionstart'));

      textarea.value = '日本';
      textarea.dispatchEvent(new Event('input')); // Intermediate — ignored

      textarea.dispatchEvent(new CompositionEvent('compositionend'));

      expect(sentToTerminal).toEqual(['日本']);
      expect(textarea.value).toBe('');
    });

    it('input events resume normal handling after composition ends', () => {
      textarea.dispatchEvent(new CompositionEvent('compositionstart'));
      textarea.value = 'あ';
      textarea.dispatchEvent(new Event('input')); // Ignored
      textarea.dispatchEvent(new CompositionEvent('compositionend'));

      // Now type a regular character
      textarea.value = 'x';
      textarea.dispatchEvent(new Event('input'));

      expect(sentToTerminal).toEqual(['あ', 'x']);
    });
  });
});
