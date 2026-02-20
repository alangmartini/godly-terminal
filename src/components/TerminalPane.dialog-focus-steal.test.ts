// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for Bug #197: Terminal steals focus from Quick Claude dialog.
 *
 * When a dialog overlay (Quick Claude, Worktree Name, etc.) is open,
 * TerminalPane.setActive(true) unconditionally schedules focusInput()
 * via RAF + 50ms setTimeout. Any store state change while a dialog is
 * open steals focus from the dialog's input, making it impossible to type.
 *
 * Root cause: setActive() has no awareness of dialog overlays. It always
 * schedules focus recovery, even when the user is interacting with a dialog.
 */

// ── Helpers ──────────────────────────────────────────────────────────────

/**
 * Simulate the TerminalPane mount setup: creates a container with a
 * hidden textarea (the keyboard input target) and the focus mechanisms
 * from setActive().
 */
function createTerminalPaneMock() {
  const container = document.createElement('div');
  container.className = 'terminal-pane';

  const inputTextarea = document.createElement('textarea');
  inputTextarea.className = 'terminal-input-hidden';
  inputTextarea.tabIndex = 0;
  inputTextarea.style.cssText =
    'position:absolute;left:-9999px;top:0;width:1px;height:1px;' +
    'opacity:0;overflow:hidden;resize:none;border:none;padding:0;' +
    'white-space:pre;z-index:-1;';
  container.appendChild(inputTextarea);

  document.body.appendChild(container);

  /**
   * Mirrors TerminalPane.focusInput() — the single choke point for all
   * focus recovery paths. Must check for dialog overlays (Bug #197 fix).
   */
  const focusInput = () => {
    if (document.querySelector('.dialog-overlay')) return;
    inputTextarea.focus();
  };

  /**
   * Mirrors TerminalPane.setActive(true) focus logic (lines 775-798).
   * This is what runs on every store state change for the active terminal.
   */
  const setActive = (active: boolean) => {
    container.classList.remove('split-visible', 'split-focused');
    container.classList.toggle('active', active);
    if (active) {
      requestAnimationFrame(() => {
        focusInput();
      });
      // Double-tap focus: backup setTimeout at 50ms
      setTimeout(() => {
        if (container.classList.contains('active')) {
          focusInput();
        }
      }, 50);
    }
  };

  return { container, inputTextarea, setActive, focusInput };
}

/**
 * Create a Quick Claude dialog overlay with a prompt textarea,
 * mirroring showQuickClaudeDialog() from dialogs.ts.
 */
function createQuickClaudeDialog(): { overlay: HTMLElement; promptArea: HTMLTextAreaElement } {
  const overlay = document.createElement('div');
  overlay.className = 'dialog-overlay';

  const dialog = document.createElement('div');
  dialog.className = 'dialog';

  const promptArea = document.createElement('textarea');
  promptArea.className = 'dialog-input';
  promptArea.placeholder = 'Describe your idea...';
  promptArea.rows = 4;
  dialog.appendChild(promptArea);

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  // Dialog focuses the textarea on open (line 370 in dialogs.ts)
  promptArea.focus();

  return { overlay, promptArea };
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #197: Terminal steals focus from Quick Claude dialog', () => {
  beforeEach(() => {
    vi.useFakeTimers({
      toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame'],
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.textContent = '';
  });

  describe('setActive steals focus from open dialog', () => {
    // Bug #197: setActive(true) unconditionally calls focusInput(), stealing
    // focus from any dialog textarea that currently has focus.

    it('should NOT steal focus from Quick Claude dialog textarea via RAF', () => {
      const pane = createTerminalPaneMock();
      const dialog = createQuickClaudeDialog();

      // Verify dialog textarea has focus
      expect(document.activeElement).toBe(dialog.promptArea);

      // Simulate a store state change triggering setActive(true).
      // This is what happens when terminal output arrives while the dialog is open.
      pane.setActive(true);

      // Run only the RAF callback (the first focus attempt).
      // advanceTimersByTime(16) triggers the faked requestAnimationFrame.
      vi.advanceTimersByTime(16);

      // Bug: the terminal's hidden textarea steals focus from the dialog
      // Expected: dialog textarea should retain focus
      expect(document.activeElement).toBe(dialog.promptArea);
    });

    it('should NOT steal focus from Quick Claude dialog textarea via 50ms setTimeout', () => {
      const pane = createTerminalPaneMock();
      const dialog = createQuickClaudeDialog();

      expect(document.activeElement).toBe(dialog.promptArea);

      // Simulate store state change
      pane.setActive(true);

      // Run all timers including the 50ms backup
      vi.advanceTimersByTime(100);

      // Bug: the 50ms setTimeout backup also steals focus
      // Expected: dialog textarea should retain focus
      expect(document.activeElement).toBe(dialog.promptArea);
    });

    it('should NOT steal focus during repeated setActive calls (multiple store changes)', () => {
      const pane = createTerminalPaneMock();
      const dialog = createQuickClaudeDialog();

      expect(document.activeElement).toBe(dialog.promptArea);

      // Simulate multiple rapid store state changes while dialog is open
      // (e.g., terminal output events arriving in quick succession)
      pane.setActive(true);
      vi.advanceTimersByTime(10);

      pane.setActive(true);
      vi.advanceTimersByTime(10);

      pane.setActive(true);
      vi.advanceTimersByTime(100);

      // Focus must remain on the dialog textarea through all the state changes
      expect(document.activeElement).toBe(dialog.promptArea);
    });
  });

  describe('focus recovery works after dialog closes', () => {
    // After the dialog is dismissed, setActive should be able to
    // reclaim focus for the terminal as before.

    it('should reclaim focus after dialog overlay is removed', () => {
      const pane = createTerminalPaneMock();
      const dialog = createQuickClaudeDialog();

      expect(document.activeElement).toBe(dialog.promptArea);

      // User dismisses the dialog (Escape or Ctrl+Enter)
      dialog.overlay.remove();

      // Next store change triggers setActive
      pane.setActive(true);
      vi.advanceTimersByTime(100);

      // Now the terminal SHOULD reclaim focus since no dialog is open
      expect(document.activeElement).toBe(pane.inputTextarea);
    });
  });

  describe('typing in dialog while terminal output arrives', () => {
    // Real-world scenario: user is typing a prompt in Quick Claude while
    // the terminal produces output. Each output event triggers a store
    // change → setActive → focusInput, interrupting every keystroke.

    it('should allow continuous typing in dialog despite store changes', () => {
      const pane = createTerminalPaneMock();
      const dialog = createQuickClaudeDialog();

      // User starts typing
      dialog.promptArea.value = 'Fix the';
      expect(document.activeElement).toBe(dialog.promptArea);

      // Terminal output arrives → store change → setActive
      pane.setActive(true);
      vi.advanceTimersByTime(20);

      // User continues typing
      dialog.promptArea.value = 'Fix the scrollback';
      expect(document.activeElement).toBe(dialog.promptArea);

      // More terminal output
      pane.setActive(true);
      vi.advanceTimersByTime(60);

      // User finishes typing
      dialog.promptArea.value = 'Fix the scrollback bug in daemon';
      expect(document.activeElement).toBe(dialog.promptArea);

      // Another output event
      pane.setActive(true);
      vi.advanceTimersByTime(100);

      // Focus must still be on the dialog textarea
      expect(document.activeElement).toBe(dialog.promptArea);
    });
  });

  describe('affects all dialog types, not just Quick Claude', () => {
    // Any dialog with a focused input will have focus stolen by the
    // terminal's setActive focus recovery.

    it('should NOT steal focus from a generic dialog input element', () => {
      const pane = createTerminalPaneMock();

      // Create a generic dialog (e.g., Worktree Name, Figma URL)
      const overlay = document.createElement('div');
      overlay.className = 'dialog-overlay';
      const input = document.createElement('input');
      input.type = 'text';
      input.className = 'dialog-input';
      overlay.appendChild(input);
      document.body.appendChild(overlay);
      input.focus();

      expect(document.activeElement).toBe(input);

      // Store change triggers setActive
      pane.setActive(true);
      vi.advanceTimersByTime(100);

      // Bug: terminal steals focus from any dialog input
      expect(document.activeElement).toBe(input);
    });
  });
});
