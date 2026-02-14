import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { activatePane } from './pane-activation';

// Bug: When switching tabs, the terminal viewport resets to the top of the
// buffer instead of staying at the bottom (where the cursor is). Even scrolling
// to what xterm.js thinks is the "bottom" shows truncated content because the
// viewport's scroll area wasn't updated for data written while the pane was
// display:none. Typing input forces a viewport refresh that reveals the real
// bottom — proving the buffer is correct but the viewport is stale.
//
// Fix: After making a pane visible, call scrollToBottom() after fit() to
// reposition the viewport at the actual end of the buffer.

describe('Terminal pane activation (scroll-to-bottom on tab switch)', () => {
  it('calls scrollToBottom after fit when pane becomes active', () => {
    const callOrder: string[] = [];
    const terminal = {
      scrollToBottom: vi.fn(() => callOrder.push('scrollToBottom')),
      focus: vi.fn(() => callOrder.push('focus')),
    };
    const fit = vi.fn(() => callOrder.push('fit'));

    activatePane(terminal, fit, true);

    // Bug: scrollToBottom was never called, so viewport stayed at top after
    // switching tabs. Data written while hidden made the buffer longer than the
    // viewport knew, so even scrolling to "bottom" showed truncated content.
    expect(terminal.scrollToBottom).toHaveBeenCalledTimes(1);
    // Correct order: fit dimensions first, then scroll, then focus
    expect(callOrder).toEqual(['fit', 'scrollToBottom', 'focus']);
  });

  it('calls scrollToBottom even when not focusing (split-visible but not focused)', () => {
    const terminal = {
      scrollToBottom: vi.fn(),
      focus: vi.fn(),
    };
    const fit = vi.fn();

    activatePane(terminal, fit, false);

    expect(fit).toHaveBeenCalledTimes(1);
    expect(terminal.scrollToBottom).toHaveBeenCalledTimes(1);
    expect(terminal.focus).not.toHaveBeenCalled();
  });

  it('scrollToBottom runs after fit so dimensions are correct first', () => {
    // Bug: without scrollToBottom, viewport has stale scroll area from when
    // the element was display:none. Data written while hidden makes the buffer
    // longer than the viewport knows, so "bottom" is truncated.
    let fitCalled = false;
    let scrollCalledAfterFit = false;

    const terminal = {
      scrollToBottom: vi.fn(() => { scrollCalledAfterFit = fitCalled; }),
      focus: vi.fn(),
    };
    const fit = vi.fn(() => { fitCalled = true; });

    activatePane(terminal, fit, true);

    expect(scrollCalledAfterFit).toBe(true);
  });
});
