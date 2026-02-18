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

  /** Mirror of TerminalPane.handleScroll */
  handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    mockSetScrollback(newOffset);
    mockFetchSnapshot();
  }

  /** Mirror of TerminalPane.snapToBottom */
  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    mockSetScrollback(0);
    mockFetchSnapshot();
  }

  /** Simulate snapshot update (as fetchAndRenderSnapshot does) */
  applySnapshot(offset: number, total: number) {
    if (!this.isUserScrolled) {
      this.scrollbackOffset = offset;
    }
    this.totalScrollback = total;
  }

  /** Simulate terminal output arriving */
  onTerminalOutput() {
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
    it('Shift+PageUp matches scroll.pageUp action', () => {
      const action = keybindingStore.matchAction(keydown('PageUp', { shiftKey: true }));
      expect(action).toBe('scroll.pageUp');
    });

    it('Shift+PageDown matches scroll.pageDown action', () => {
      const action = keybindingStore.matchAction(keydown('PageDown', { shiftKey: true }));
      expect(action).toBe('scroll.pageDown');
    });

    it('Shift+Home matches scroll.toTop action', () => {
      const action = keybindingStore.matchAction(keydown('Home', { shiftKey: true }));
      expect(action).toBe('scroll.toTop');
    });

    it('Shift+End matches scroll.toBottom action', () => {
      const action = keybindingStore.matchAction(keydown('End', { shiftKey: true }));
      expect(action).toBe('scroll.toBottom');
    });

    it('bare PageUp does NOT match any action (passes through to PTY)', () => {
      const action = keybindingStore.matchAction(keydown('PageUp'));
      expect(action).toBeNull();
    });

    it('bare PageDown does NOT match any action (passes through to PTY)', () => {
      const action = keybindingStore.matchAction(keydown('PageDown'));
      expect(action).toBeNull();
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
        keybindingStore.isAppShortcut(keydown('PageUp', { shiftKey: true }))
      ).toBe(true);
    });

    it('scroll.pageUp is NOT a terminal control key', () => {
      expect(
        keybindingStore.isTerminalControlKey(keydown('PageUp', { shiftKey: true }))
      ).toBe(false);
    });
  });
});
