import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { keybindingStore } from '../state/keybinding-store';
import { terminalSettingsStore } from '../state/terminal-settings-store';

/**
 * Tests for scrollback handling in TerminalPane.
 *
 * These tests verify the scroll decision logic without a full Tauri/canvas
 * environment. They mirror the keyboard→scroll routing from TerminalPane.ts.
 */

// ── Mock terminal service ───────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);
const mockFetchSnapshot = vi.fn().mockResolvedValue(undefined);

// ── Scroll state simulator ──────────────────────────────────────────────

/** Simulates the scroll state management from TerminalPane. */
class ScrollSimulator {
  scrollbackOffset = 0;
  totalScrollback = 0;
  gridRows = 24;
  isUserScrolled = false;

  // Simulates renderer.isActivelySelecting() — true while user is dragging
  rendererIsSelecting = false;

  // scrollSeq counter mirrors TerminalPane for race-condition testing
  scrollSeq = 0;

  /** Mirror of TerminalPane.handleScroll */
  handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    const seq = ++this.scrollSeq;
    mockSetScrollback(newOffset);
    // Simulate async fetch that checks seq
    return { seq, fetch: () => {
      if (seq === this.scrollSeq) {
        mockFetchSnapshot();
      }
    }};
  }

  /** Mirror of TerminalPane.handleScrollTo (absolute offset from scrollbar drag) */
  handleScrollTo(absoluteOffset: number) {
    const newOffset = Math.max(0, Math.round(absoluteOffset));
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    const seq = ++this.scrollSeq;
    mockSetScrollback(newOffset);
    return { seq, fetch: () => {
      if (seq === this.scrollSeq) {
        mockFetchSnapshot();
      }
    }};
  }

  /** Mirror of TerminalPane.snapToBottom */
  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    const seq = ++this.scrollSeq;
    mockSetScrollback(0);
    return { seq, fetch: () => {
      if (seq === this.scrollSeq) {
        mockFetchSnapshot();
      }
    }};
  }

  /** Simulate snapshot update (as fetchAndRenderSnapshot does) */
  applySnapshot(offset: number, total: number) {
    if (!this.isUserScrolled) {
      this.scrollbackOffset = offset;
    }
    this.totalScrollback = total;
  }

  /** Simulate terminal output arriving (mirrors output handler) */
  onTerminalOutput() {
    // Freeze display while the user is dragging to select text
    if (this.rendererIsSelecting) return;
    if (this.isUserScrolled && terminalSettingsStore.getAutoScrollOnOutput()) {
      this.snapToBottom();
      return;
    }
    mockFetchSnapshot();
  }
}

// ── Helpers ──────────────────────────────────────────────────────────────

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

// ── Tests ────────────────────────────────────────────────────────────────

describe('TerminalPane scroll handling', () => {
  let sim: ScrollSimulator;

  beforeEach(() => {
    sim = new ScrollSimulator();
    mockSetScrollback.mockClear();
    mockFetchSnapshot.mockClear();
  });

  afterEach(() => {
    keybindingStore.resetAll();
    terminalSettingsStore.setAutoScrollOnOutput(false);
  });

  describe('keyboard shortcut routing', () => {
    it('bare PageUp matches scroll.pageUp action', () => {
      const action = keybindingStore.matchAction(keydown('PageUp'));
      expect(action).toBe('scroll.pageUp');
    });

    it('bare PageDown matches scroll.pageDown action', () => {
      const action = keybindingStore.matchAction(keydown('PageDown'));
      expect(action).toBe('scroll.pageDown');
    });

    it('Ctrl+Shift+Home matches scroll.toTop action', () => {
      const action = keybindingStore.matchAction(keydown('Home', { ctrlKey: true, shiftKey: true }));
      expect(action).toBe('scroll.toTop');
    });

    it('Ctrl+Shift+End matches scroll.toBottom action', () => {
      const action = keybindingStore.matchAction(keydown('End', { ctrlKey: true, shiftKey: true }));
      expect(action).toBe('scroll.toBottom');
    });

    it('bare Home does NOT match scroll action (sends to PTY instead)', () => {
      const action = keybindingStore.matchAction(keydown('Home'));
      expect(action).toBeNull();
    });

    it('bare End does NOT match scroll action (sends to PTY instead)', () => {
      const action = keybindingStore.matchAction(keydown('End'));
      expect(action).toBeNull();
    });

    it('Shift+PageUp does NOT match scroll action (keys changed to bare)', () => {
      const action = keybindingStore.matchAction(keydown('PageUp', { shiftKey: true }));
      expect(action).toBeNull();
    });

    it('Shift+PageDown does NOT match scroll action (keys changed to bare)', () => {
      const action = keybindingStore.matchAction(keydown('PageDown', { shiftKey: true }));
      expect(action).toBeNull();
    });
  });

  describe('alternate screen pass-through', () => {
    // When on alternate screen (vim, less, htop), scroll shortcuts should
    // NOT be intercepted — they pass through to the PTY app. This is
    // verified at the TerminalPane.handleKeyEvent level; here we just
    // confirm the keybinding store still matches so the guard logic in
    // handleKeyEvent is what decides.
    it('scroll.pageUp action still matches bare PageUp (guard is in TerminalPane)', () => {
      expect(keybindingStore.matchAction(keydown('PageUp'))).toBe('scroll.pageUp');
    });

    it('scroll.toTop action matches Ctrl+Shift+Home (guard is in TerminalPane)', () => {
      expect(keybindingStore.matchAction(keydown('Home', { ctrlKey: true, shiftKey: true }))).toBe('scroll.toTop');
    });
  });

  describe('scroll.pageUp action', () => {
    it('scrolls up by one page (gridRows lines)', () => {
      sim.totalScrollback = 100;
      sim.handleScroll(sim.gridRows); // pageUp
      expect(mockSetScrollback).toHaveBeenCalledWith(24);
    });

    it('does nothing when already at offset 0 and deltaLines is negative', () => {
      sim.handleScroll(-5);
      expect(mockSetScrollback).not.toHaveBeenCalled();
    });
  });

  describe('scroll.pageDown action', () => {
    it('scrolls down by one page', () => {
      sim.scrollbackOffset = 50;
      sim.totalScrollback = 100;
      sim.handleScroll(-sim.gridRows); // pageDown
      expect(mockSetScrollback).toHaveBeenCalledWith(26);
    });

    it('clamps to 0 when scrolling past bottom', () => {
      sim.scrollbackOffset = 10;
      sim.totalScrollback = 100;
      sim.handleScroll(-20); // more than offset
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
    });
  });

  describe('scroll.toTop action', () => {
    it('scrolls to top of scrollback', () => {
      sim.totalScrollback = 500;
      sim.handleScroll(sim.totalScrollback);
      expect(mockSetScrollback).toHaveBeenCalledWith(500);
    });
  });

  describe('scroll.toBottom action', () => {
    it('snaps to live view', () => {
      sim.scrollbackOffset = 200;
      sim.snapToBottom();
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
    });

    it('does nothing when already at bottom', () => {
      sim.scrollbackOffset = 0;
      sim.snapToBottom();
      expect(mockSetScrollback).not.toHaveBeenCalled();
    });
  });

  describe('wheel scroll', () => {
    it('positive delta (scroll up into history) increases offset', () => {
      sim.totalScrollback = 100;
      sim.handleScroll(3); // wheel up = 3 lines
      expect(mockSetScrollback).toHaveBeenCalledWith(3);
    });

    it('negative delta (scroll down toward live) decreases offset', () => {
      sim.scrollbackOffset = 10;
      sim.handleScroll(-3); // wheel down
      expect(mockSetScrollback).toHaveBeenCalledWith(7);
    });

    it('clamps offset to 0 on scroll down past bottom', () => {
      sim.scrollbackOffset = 2;
      sim.handleScroll(-5);
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
    });
  });

  describe('scrollbar drag (handleScrollTo)', () => {
    it('sets absolute offset directly', () => {
      sim.totalScrollback = 500;
      sim.handleScrollTo(250);
      expect(mockSetScrollback).toHaveBeenCalledWith(250);
      expect(sim.scrollbackOffset).toBe(250);
    });

    it('clamps to 0 for negative values', () => {
      sim.totalScrollback = 500;
      sim.scrollbackOffset = 50; // must be non-zero so clamped-to-0 is a change
      sim.handleScrollTo(-10);
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
    });

    it('rounds fractional offsets', () => {
      sim.totalScrollback = 500;
      sim.handleScrollTo(123.7);
      expect(mockSetScrollback).toHaveBeenCalledWith(124);
    });

    it('does nothing when offset is unchanged', () => {
      sim.scrollbackOffset = 100;
      sim.handleScrollTo(100);
      expect(mockSetScrollback).not.toHaveBeenCalled();
    });
  });

  describe('scroll race condition (scrollSeq guard)', () => {
    it('discards stale fetch when a newer scroll happened', () => {
      sim.totalScrollback = 1000;

      // First scroll: offset 50
      const first = sim.handleScroll(50);
      expect(sim.scrollSeq).toBe(1);

      // Second scroll before first fetch completes: offset 100
      sim.handleScroll(50);
      expect(sim.scrollSeq).toBe(2);

      // First fetch completes — should be discarded (stale seq)
      first!.fetch();
      expect(mockFetchSnapshot).not.toHaveBeenCalled();
    });

    it('accepts fetch when no newer scroll happened', () => {
      sim.totalScrollback = 1000;
      const result = sim.handleScroll(50);
      result!.fetch();
      expect(mockFetchSnapshot).toHaveBeenCalledTimes(1);
    });

    it('scrollSeq increments across handleScroll and handleScrollTo', () => {
      sim.totalScrollback = 1000;
      sim.handleScroll(10);      // seq=1
      sim.handleScrollTo(200);   // seq=2
      sim.handleScroll(10);      // seq=3
      expect(sim.scrollSeq).toBe(3);
    });

    it('snapToBottom increments scrollSeq and guards stale fetches', () => {
      sim.totalScrollback = 1000;
      sim.scrollbackOffset = 50;
      const scrollResult = sim.handleScroll(10);  // seq=1
      sim.snapToBottom();                          // seq=2

      // Stale scroll fetch
      scrollResult!.fetch();
      expect(mockFetchSnapshot).not.toHaveBeenCalled();
    });
  });

  describe('snap-to-bottom on keypress', () => {
    it('snaps to bottom when scrolled up and user types', () => {
      sim.scrollbackOffset = 50;
      sim.snapToBottom(); // simulates what handleKeyEvent does before sending input
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
      expect(sim.scrollbackOffset).toBe(0);
    });

    it('does not snap when already at bottom', () => {
      sim.scrollbackOffset = 0;
      sim.snapToBottom();
      expect(mockSetScrollback).not.toHaveBeenCalled();
    });
  });

  describe('snapshot state tracking', () => {
    it('applySnapshot updates local scroll state from daemon', () => {
      sim.applySnapshot(42, 1000);
      expect(sim.scrollbackOffset).toBe(42);
      expect(sim.totalScrollback).toBe(1000);
    });

    it('scroll after snapshot uses updated state', () => {
      sim.applySnapshot(42, 1000);
      sim.handleScroll(10); // scroll up 10 more
      expect(mockSetScrollback).toHaveBeenCalledWith(52);
    });
  });

  describe('scroll position preservation (race condition fix)', () => {
    it('sets isUserScrolled when scrolling up', () => {
      sim.handleScroll(10);
      expect(sim.isUserScrolled).toBe(true);
    });

    it('clears isUserScrolled when snapping to bottom', () => {
      sim.handleScroll(10);
      sim.snapToBottom();
      expect(sim.isUserScrolled).toBe(false);
    });

    it('clears isUserScrolled when scrolling back to offset 0', () => {
      sim.scrollbackOffset = 3;
      sim.isUserScrolled = true;
      sim.handleScroll(-3);
      expect(sim.isUserScrolled).toBe(false);
    });

    it('preserves scroll position when snapshot arrives during user scroll', () => {
      // User scrolls up to offset 50
      sim.handleScroll(50);
      expect(sim.scrollbackOffset).toBe(50);
      expect(sim.isUserScrolled).toBe(true);

      // Daemon returns a snapshot with offset 0 (race: setScrollback not yet processed)
      sim.applySnapshot(0, 1000);

      // Scroll position should NOT be overwritten
      expect(sim.scrollbackOffset).toBe(50);
      // totalScrollback SHOULD still update
      expect(sim.totalScrollback).toBe(1000);
    });

    it('allows snapshot to update offset when not user-scrolled', () => {
      // Not scrolled up — daemon snapshot should update offset normally
      sim.applySnapshot(0, 500);
      expect(sim.scrollbackOffset).toBe(0);
      expect(sim.totalScrollback).toBe(500);
    });

    it('allows snapshot to update offset after snap-to-bottom', () => {
      sim.handleScroll(30);
      sim.snapToBottom();

      // Now a snapshot arrives — should update normally
      sim.applySnapshot(0, 800);
      expect(sim.scrollbackOffset).toBe(0);
    });
  });

  describe('auto-scroll on output setting', () => {
    it('does not snap to bottom on output when disabled (default)', () => {
      sim.handleScroll(50);
      mockSetScrollback.mockClear();
      mockFetchSnapshot.mockClear();

      sim.onTerminalOutput();

      // Should NOT have called snapToBottom (no setScrollback(0))
      expect(mockSetScrollback).not.toHaveBeenCalled();
      // Should still fetch snapshot
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });

    it('snaps to bottom on output when enabled and user is scrolled up', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);
      sim.handleScroll(50);
      mockSetScrollback.mockClear();
      mockFetchSnapshot.mockClear();

      sim.onTerminalOutput();

      expect(mockSetScrollback).toHaveBeenCalledWith(0);
      expect(sim.scrollbackOffset).toBe(0);
      expect(sim.isUserScrolled).toBe(false);
    });

    it('does not snap when enabled but already at bottom', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);
      mockSetScrollback.mockClear();
      mockFetchSnapshot.mockClear();

      sim.onTerminalOutput();

      // Already at bottom, no snap needed — just fetch snapshot
      expect(mockSetScrollback).not.toHaveBeenCalled();
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });
  });

  describe('scroll shortcuts are app-type (not terminal-control)', () => {
    it('scroll.pageUp is an app shortcut', () => {
      expect(
        keybindingStore.isAppShortcut(keydown('PageUp'))
      ).toBe(true);
    });

    it('scroll.pageUp is NOT a terminal control key', () => {
      expect(
        keybindingStore.isTerminalControlKey(keydown('PageUp'))
      ).toBe(false);
    });
  });

  describe('display freeze during text selection', () => {
    it('skips rendering when user is actively selecting text', () => {
      sim.rendererIsSelecting = true;
      sim.onTerminalOutput();
      expect(mockFetchSnapshot).not.toHaveBeenCalled();
    });

    it('resumes rendering after selection ends (mouseup)', () => {
      sim.rendererIsSelecting = true;
      sim.onTerminalOutput();
      expect(mockFetchSnapshot).not.toHaveBeenCalled();

      sim.rendererIsSelecting = false;
      sim.onTerminalOutput();
      expect(mockFetchSnapshot).toHaveBeenCalledTimes(1);
    });

    it('skips auto-scroll snap-to-bottom while selecting', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);
      sim.handleScroll(50);
      mockSetScrollback.mockClear();
      mockFetchSnapshot.mockClear();

      sim.rendererIsSelecting = true;
      sim.onTerminalOutput();

      expect(mockSetScrollback).not.toHaveBeenCalled();
      expect(sim.scrollbackOffset).toBe(50);
      expect(sim.isUserScrolled).toBe(true);
    });

    it('multiple output events during selection are all suppressed', () => {
      sim.rendererIsSelecting = true;
      sim.onTerminalOutput();
      sim.onTerminalOutput();
      sim.onTerminalOutput();
      expect(mockFetchSnapshot).not.toHaveBeenCalled();
      expect(mockSetScrollback).not.toHaveBeenCalled();
    });

    it('catches up after selection with auto-scroll enabled', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);
      sim.handleScroll(50);
      mockSetScrollback.mockClear();
      mockFetchSnapshot.mockClear();

      sim.rendererIsSelecting = true;
      sim.onTerminalOutput();
      expect(mockSetScrollback).not.toHaveBeenCalled();

      sim.rendererIsSelecting = false;
      sim.onTerminalOutput();
      expect(mockSetScrollback).toHaveBeenCalledWith(0);
    });
  });
});
