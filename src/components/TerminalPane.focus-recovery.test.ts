// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for the canvas focus recovery mechanism in TerminalPane.
 *
 * Bug: A single terminal tab becomes completely unresponsive to keyboard
 * input (Ctrl+C, Esc, typing all fail) while the daemon, bridge, and
 * session are healthy. The root cause: the canvas loses focus and there
 * is no mechanism to recover it.
 *
 * Root cause: The container mousedown handler only focused the canvas in
 * split mode (checking `split-visible` class). In single-pane mode,
 * focus recovery relied solely on browser default behavior, which fails
 * after tab bar clicks, dialog dismissals, or WebView2 native frame
 * focus events steal focus from the canvas.
 *
 * Fix:
 * 1. Container mousedown always focuses canvas (not just in split mode)
 * 2. setActive() uses double-tap focus (RAF + setTimeout 50ms backup)
 * 3. Blur diagnostic logging detects future focus theft
 */

// ── Helpers ──────────────────────────────────────────────────────────────

/** Minimal mock of TerminalRenderer's focus-related API */
function createMockRenderer() {
  const focusCalls: number[] = [];
  return {
    focusCalls,
    focus: vi.fn(() => focusCalls.push(Date.now())),
    getElement: vi.fn(() => document.createElement('canvas')),
    getOverlayElement: vi.fn(() => null),
    updateSize: vi.fn(),
    scrollToBottom: vi.fn(),
    getGridSize: vi.fn(() => ({ rows: 24, cols: 80 })),
  };
}

/** Simulate the mount-time mousedown handler from TerminalPane.ts:89-96 */
function simulateContainerMousedownHandler(
  container: HTMLElement,
  renderer: ReturnType<typeof createMockRenderer>,
  setActiveTerminal: (id: string) => void,
  terminalId: string
) {
  container.addEventListener('mousedown', () => {
    if (container.classList.contains('split-visible')) {
      setActiveTerminal(terminalId);
    }
    // This is the fix: always focus the canvas via RAF
    requestAnimationFrame(() => renderer.focus());
  });
}

/** Simulate the setActive(true) focus logic from TerminalPane.ts:500-523 */
function simulateSetActive(
  container: HTMLElement,
  renderer: ReturnType<typeof createMockRenderer>,
  active: boolean
) {
  container.classList.remove('split-visible', 'split-focused');
  container.classList.toggle('active', active);
  if (active) {
    renderer.updateSize();
    requestAnimationFrame(() => {
      renderer.scrollToBottom();
      renderer.focus();
    });
    // Double-tap focus
    setTimeout(() => {
      if (container.classList.contains('active')) {
        renderer.focus();
      }
    }, 50);
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('TerminalPane focus recovery (keyboard input freeze fix)', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame'] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('Container mousedown always focuses canvas', () => {
    // Bug: Previously, mousedown only focused canvas in split mode.
    // In single-pane mode, clicking the terminal area after focus was
    // stolen (by tab bar, dialog, etc.) left the canvas unfocused.

    it('focuses canvas on mousedown in single-pane mode (no split-visible class)', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');
      container.className = 'terminal-pane active';

      simulateContainerMousedownHandler(container, renderer, vi.fn(), 'term-1');

      container.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));

      // RAF callback runs focus
      vi.runAllTimers();
      expect(renderer.focus).toHaveBeenCalledTimes(1);
    });

    it('focuses canvas on mousedown in split mode (with split-visible class)', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');
      container.className = 'terminal-pane split-visible';
      const setActive = vi.fn();

      simulateContainerMousedownHandler(container, renderer, setActive, 'term-1');

      container.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));

      vi.runAllTimers();
      expect(renderer.focus).toHaveBeenCalledTimes(1);
      expect(setActive).toHaveBeenCalledWith('term-1');
    });

    it('recovers focus after focus was stolen by another element', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');
      container.className = 'terminal-pane active';

      simulateContainerMousedownHandler(container, renderer, vi.fn(), 'term-1');

      // Simulate focus theft (e.g., tab bar click moved focus to body)
      document.body.focus();

      // User clicks back on the terminal area
      container.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));

      vi.runAllTimers();
      expect(renderer.focus).toHaveBeenCalledTimes(1);
    });
  });

  describe('setActive double-tap focus', () => {
    // Bug: A single RAF focus in setActive(true) can be stolen by
    // WebView2 native frame focus events or tab bar click cleanup
    // that fires after the RAF callback.

    it('calls focus twice when activated: once in RAF, once in setTimeout', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');

      simulateSetActive(container, renderer, true);

      // Flush all pending timers (RAF + setTimeout 50ms)
      vi.runAllTimers();
      // Both the RAF focus and the setTimeout(50ms) backup should have fired
      expect(renderer.focus).toHaveBeenCalledTimes(2);
    });

    it('does not call backup focus if pane was deactivated before timeout', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');

      simulateSetActive(container, renderer, true);

      // Deactivate before any timers fire
      container.classList.remove('active');

      // Flush all pending timers
      vi.runAllTimers();
      // RAF focus fires unconditionally, but setTimeout backup checks
      // container.classList.contains('active') and skips when inactive.
      // So only the RAF focus call should have fired.
      expect(renderer.focus).toHaveBeenCalledTimes(1);
    });

    it('does not call focus when setActive(false)', () => {
      const renderer = createMockRenderer();
      const container = document.createElement('div');
      container.className = 'terminal-pane active';

      simulateSetActive(container, renderer, false);

      vi.advanceTimersByTime(100);
      expect(renderer.focus).not.toHaveBeenCalled();
    });
  });

  describe('Blur diagnostic logging', () => {
    // The blur handler logs a warning when the canvas loses focus while
    // the pane is active. This doesn't fix the freeze, but helps
    // diagnose which element stole focus.

    it('logs warning when canvas blurs while pane is active', () => {
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const canvas = document.createElement('canvas');
      const container = document.createElement('div');
      container.className = 'terminal-pane active';
      container.appendChild(canvas);
      const terminalId = 'term-diag';

      // Simulate the blur handler from TerminalPane.ts:120-129
      canvas.addEventListener('blur', () => {
        if (container.classList.contains('active') ||
            container.classList.contains('split-focused')) {
          const thief = document.activeElement;
          console.warn(
            `[TerminalPane] Canvas lost focus while active (terminal=${terminalId}, ` +
            `now focused: ${thief?.tagName}${thief?.className ? '.' + thief.className : ''})`
          );
        }
      });

      canvas.dispatchEvent(new FocusEvent('blur'));

      expect(warnSpy).toHaveBeenCalledTimes(1);
      expect(warnSpy.mock.calls[0][0]).toContain('[TerminalPane] Canvas lost focus while active');
      expect(warnSpy.mock.calls[0][0]).toContain('term-diag');

      warnSpy.mockRestore();
    });

    it('does not log warning when canvas blurs while pane is inactive', () => {
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const canvas = document.createElement('canvas');
      const container = document.createElement('div');
      container.className = 'terminal-pane'; // no 'active' class
      container.appendChild(canvas);

      canvas.addEventListener('blur', () => {
        if (container.classList.contains('active') ||
            container.classList.contains('split-focused')) {
          console.warn('[TerminalPane] Canvas lost focus while active');
        }
      });

      canvas.dispatchEvent(new FocusEvent('blur'));

      expect(warnSpy).not.toHaveBeenCalled();
      warnSpy.mockRestore();
    });
  });
});
