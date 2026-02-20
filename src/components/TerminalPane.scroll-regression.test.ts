import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { terminalSettingsStore } from '../state/terminal-settings-store';

/**
 * Bug #202 regression: Auto-scroll to bottom during sustained output.
 *
 * The original #202 fix prevented snapshot fetches from overwriting the
 * scroll position. However, three additional code paths still cause
 * the viewport to snap to bottom during heavy output (e.g., Claude Code):
 *
 * 1. OFFSET DRIFT: The frontend's scrollbackOffset stays frozen at the
 *    user's original scroll position while the daemon increments its offset
 *    with each new line. Subsequent scroll operations compute deltas from
 *    the stale frontend value, causing massive viewport jumps.
 *    (TerminalPane.ts:474-487, 594-600, 687-689)
 *
 * 2. KEYBOARD INPUT SNAP: Any keystroke while scrolled up triggers
 *    snapToBottom() unconditionally (TerminalPane.ts:364), even when the
 *    user intends to type while reviewing scrollback.
 *
 * 3. PANE ACTIVATION SNAP: Switching tabs or split panes calls
 *    renderer.scrollToBottom() (TerminalPane.ts:786,811), which resets
 *    the scroll position even if the user was intentionally scrolled up.
 *
 * Run: npx vitest run src/components/TerminalPane.scroll-regression.test.ts
 */

// ── Mock services ────────────────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * Extended scroll state simulator that tracks both frontend and daemon offsets.
 * Mirrors the real TerminalPane scroll state machine including the drift bug.
 */
class ScrollRegressionSimulator {
  // Frontend state (TerminalPane fields)
  scrollbackOffset = 0;
  totalScrollback = 0;
  isUserScrolled = false;
  scrollSeq = 0;
  cachedSnapshot: {
    scrollback_offset: number;
    total_scrollback: number;
    alternate_screen: boolean;
  } | null = null;

  // Tracks what the daemon's actual offset would be
  daemonOffset = 0;

  /** Mirror of TerminalPane.handleScroll (line 474) */
  handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    ++this.scrollSeq;
    this.cachedSnapshot = null;
    mockSetScrollback(newOffset);
    this.daemonOffset = newOffset;
  }

  /** Mirror of TerminalPane.handleScrollTo (line 493) */
  handleScrollTo(absoluteOffset: number) {
    const newOffset = Math.max(0, Math.round(absoluteOffset));
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    ++this.scrollSeq;
    this.cachedSnapshot = null;
    mockSetScrollback(newOffset);
    this.daemonOffset = newOffset;
  }

  /** Mirror of TerminalPane.snapToBottom (line 508) */
  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    ++this.scrollSeq;
    this.cachedSnapshot = null;
    mockSetScrollback(0);
    this.daemonOffset = 0;
  }

  /**
   * Simulate pushed diff arriving from daemon after new output.
   * The daemon's offset increments by 1 for each new line (scroll_up in godly-vt).
   * Mirrors the grid diff event handler + applyPushedDiff logic
   * (TerminalPane.ts lines 586-600, 680-692).
   */
  onNewOutputDiff(linesAdded: number = 1) {
    // Daemon increments offset as new lines scroll in (grid.rs scroll_up)
    if (this.daemonOffset > 0) {
      this.daemonOffset += linesAdded;
    }
    const newTotal = this.totalScrollback + linesAdded;

    if (this.isUserScrolled && terminalSettingsStore.getAutoScrollOnOutput()) {
      this.snapToBottom();
      return;
    }

    if (!this.cachedSnapshot) {
      return;
    }

    // Apply diff to cache (mirrors applyPushedDiff)
    this.cachedSnapshot.scrollback_offset = this.daemonOffset;
    this.cachedSnapshot.total_scrollback = newTotal;

    // Bug #202: only sync offset when not user-scrolled (line 687-689)
    if (!this.isUserScrolled) {
      this.scrollbackOffset = this.daemonOffset;
    }
    this.totalScrollback = newTotal;
  }

  /** Simulate snapshot arriving from daemon */
  applySnapshot(offset: number, total: number) {
    this.cachedSnapshot = {
      scrollback_offset: offset,
      total_scrollback: total,
      alternate_screen: false,
    };
    if (!this.isUserScrolled) {
      this.scrollbackOffset = offset;
    }
    this.totalScrollback = total;
    this.daemonOffset = offset;
  }

  /**
   * Mirror of renderer.scrollToBottom() (TerminalRenderer.ts:414-420)
   * → calls handleScroll(-currentOffset).
   * Called during pane activation (setActive line 786, setSplitVisible line 811).
   */
  rendererScrollToBottom() {
    const currentOffset = this.cachedSnapshot?.scrollback_offset ?? 0;
    if (currentOffset > 0) {
      this.handleScroll(-currentOffset);
    }
  }

  /**
   * Mirror of handleKeyEvent snap-to-bottom logic (TerminalPane.ts:364-366).
   * Any non-modifier keystroke while scrolled up triggers snapToBottom.
   */
  onKeyboardInput() {
    if (this.scrollbackOffset > 0) {
      this.snapToBottom();
    }
  }

  /** Get the jump magnitude that would result from a scroll delta */
  getEffectiveScrollDelta(deltaLines: number): number {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    return newOffset - this.daemonOffset;
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #202 regression: scroll-to-bottom during sustained output', () => {
  let sim: ScrollRegressionSimulator;

  beforeEach(() => {
    sim = new ScrollRegressionSimulator();
    mockSetScrollback.mockClear();
  });

  afterEach(() => {
    terminalSettingsStore.setAutoScrollOnOutput(false);
  });

  // ── 1. Offset drift ──────────────────────────────────────────────────

  describe('offset drift between frontend and daemon', () => {
    it('Bug #202: frontend offset diverges from daemon during sustained output', () => {
      // User scrolls up 10 lines
      sim.handleScroll(10);
      sim.applySnapshot(10, 500);

      // 100 lines of output while user is scrolled up
      for (let i = 0; i < 100; i++) {
        sim.onNewOutputDiff(1);
      }

      // Bug: frontend stays at 10 while daemon incremented to 110.
      // They SHOULD stay in sync for correct scroll behavior.
      expect(sim.daemonOffset).toBe(110);
      expect(sim.scrollbackOffset).toBe(sim.daemonOffset);
    });

    it('Bug #202: scroll-down-by-1 causes massive jump after offset drift', () => {
      sim.handleScroll(10);
      sim.applySnapshot(10, 500);

      // 100 lines of output → daemon drifts to 110
      for (let i = 0; i < 100; i++) {
        sim.onNewOutputDiff(1);
      }

      // User scrolls down by 1 line — expects to move 1 line toward bottom
      const deltaToSend = sim.getEffectiveScrollDelta(-1);

      // Bug: delta should be -1 (1 line down), but frontend computes from
      // stale offset (10-1=9), so daemon jumps from 110→9 = 101-line jump!
      expect(Math.abs(deltaToSend)).toBeLessThanOrEqual(2);
    });

    it('Bug #202: scroll-up-by-1 after drift sends stale offset to daemon', () => {
      sim.handleScroll(20);
      sim.applySnapshot(20, 500);

      // 200 lines of output → daemon drifts to 220
      for (let i = 0; i < 200; i++) {
        sim.onNewOutputDiff(1);
      }

      // User scrolls up by 1 more line
      // Frontend computes: 20 + 1 = 21, sends setScrollback(21)
      // Daemon was at 220, jumps to 21 = 199-line jump toward bottom!
      sim.handleScroll(1);

      const sentOffset =
        mockSetScrollback.mock.calls[mockSetScrollback.mock.calls.length - 1][0];
      // Should be ~221 (daemon's 220 + 1), not 21 (frontend's stale 20 + 1)
      expect(sentOffset).toBeGreaterThan(200);
    });

    it('Bug #202: scrollbar position diverges from frontend state after drift', () => {
      sim.handleScroll(10);
      sim.applySnapshot(10, 200);

      // 300 new lines
      for (let i = 0; i < 300; i++) {
        sim.onNewOutputDiff(1);
      }

      // Scrollbar reads cachedSnapshot.scrollback_offset (310)
      // Frontend thinks it's at offset 10 — they should match
      const scrollbarOffset = sim.cachedSnapshot!.scrollback_offset;
      expect(sim.scrollbackOffset).toBe(scrollbarOffset);
    });
  });

  // ── 2. Keyboard input snap-to-bottom ─────────────────────────────────

  describe('keyboard input unconditionally snaps to bottom', () => {
    it('Bug #202: typing while scrolled up destroys scroll position', () => {
      sim.handleScroll(50);
      sim.applySnapshot(50, 1000);
      expect(sim.isUserScrolled).toBe(true);

      // User types while viewing scrollback
      sim.onKeyboardInput();

      // Bug: keyboard input should NOT snap to bottom when user is
      // intentionally reviewing scrollback
      expect(sim.scrollbackOffset).toBe(50);
      expect(sim.isUserScrolled).toBe(true);
    });

    it('Bug #202: every keystroke during sustained output triggers snap', () => {
      sim.handleScroll(30);
      sim.applySnapshot(30, 500);

      // Output arriving while user reads scrollback
      for (let i = 0; i < 50; i++) {
        sim.onNewOutputDiff(1);
      }
      expect(sim.isUserScrolled).toBe(true);

      sim.onKeyboardInput();

      // Bug: viewport snaps to bottom, user loses their place
      expect(sim.scrollbackOffset).toBeGreaterThan(0);
    });

    it('Bug #202: typing loses position even with autoScrollOnOutput disabled', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.handleScroll(20);
      sim.applySnapshot(20, 300);

      // Output arrives, doesn't snap (correct — autoScroll is off)
      sim.onNewOutputDiff(10);
      expect(sim.scrollbackOffset).toBe(20);

      // But typing DOES snap — inconsistent with autoScrollOnOutput=false
      sim.onKeyboardInput();

      // Bug: if autoScrollOnOutput is disabled, keyboard input should
      // also not snap to bottom
      expect(sim.scrollbackOffset).toBe(20);
    });
  });

  // ── 3. Pane activation snap ──────────────────────────────────────────

  describe('pane activation resets scroll position', () => {
    it('Bug #202: switching tabs resets scroll via scrollToBottom', () => {
      sim.handleScroll(40);
      sim.applySnapshot(40, 800);
      expect(sim.isUserScrolled).toBe(true);

      // setActive calls renderer.scrollToBottom() (line 786)
      sim.rendererScrollToBottom();

      // Bug: tab switch should preserve user's scroll position
      expect(sim.scrollbackOffset).toBe(40);
      expect(sim.isUserScrolled).toBe(true);
    });

    it('Bug #202: pane activation with drifted offset guarantees snap to bottom', () => {
      sim.handleScroll(10);
      sim.applySnapshot(10, 200);

      // Heavy output → daemon drifts to 110
      for (let i = 0; i < 100; i++) {
        sim.onNewOutputDiff(1);
      }

      // renderer.scrollToBottom uses cachedSnapshot.scrollback_offset (110)
      // handleScroll(-110) → newOffset = max(0, 10 + (-110)) = 0
      sim.rendererScrollToBottom();

      // Bug: massive delta from drifted offset guarantees snap to 0
      expect(sim.scrollbackOffset).toBeGreaterThan(0);
    });
  });

  // ── 4. Combined scenario (Claude Code usage pattern) ─────────────────

  describe('Claude Code usage pattern', () => {
    it('Bug #202: full session — scroll up, output, type, lose position', () => {
      // 1. Claude Code produces initial output
      sim.applySnapshot(0, 100);

      // 2. User scrolls up to review
      sim.handleScroll(30);
      sim.applySnapshot(30, 100);
      expect(sim.scrollbackOffset).toBe(30);

      // 3. Claude Code continues generating output (100 more lines)
      for (let i = 0; i < 100; i++) {
        sim.onNewOutputDiff(1);
      }

      // 4. User types a response — any keystroke snaps to bottom
      const offsetBeforeType = sim.scrollbackOffset;
      sim.onKeyboardInput();

      // Bug: typing should not destroy scroll position
      expect(sim.scrollbackOffset).toBe(offsetBeforeType);
    });
  });
});
