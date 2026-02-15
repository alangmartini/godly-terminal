import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { keybindingStore } from '../state/keybinding-store';

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

  /** Mirror of TerminalPane.handleScroll */
  handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    mockSetScrollback(newOffset);
    mockFetchSnapshot();
  }

  /** Mirror of TerminalPane.snapToBottom */
  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    mockSetScrollback(0);
    mockFetchSnapshot();
  }

  /** Simulate snapshot update (as fetchAndRenderSnapshot does) */
  applySnapshot(offset: number, total: number) {
    this.scrollbackOffset = offset;
    this.totalScrollback = total;
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
