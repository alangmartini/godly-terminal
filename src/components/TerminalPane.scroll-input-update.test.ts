import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { terminalSettingsStore } from '../state/terminal-settings-store';

/**
 * Bug #238: Terminal display doesn't update when typing while scrolled up.
 *
 * When the user scrolls up even slightly and types, the terminal display
 * doesn't update to show the typed characters. The echo appears at the live
 * view (offset 0) but the viewport remains at the scrollback position.
 *
 * This is a regression from the #202 fix that removed unconditional
 * snapToBottom() on keystroke. The fix correctly prevents viewport loss
 * during scrollback review, but also prevents the user from seeing their
 * typed echo. There's no distinction between user-initiated echo (typing)
 * and program-generated output.
 *
 * Run: npx vitest run src/components/TerminalPane.scroll-input-update.test.ts
 */

// ── Mock services ────────────────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);
const mockFetchSnapshot = vi.fn().mockResolvedValue(undefined);
const mockRender = vi.fn();

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * Simulates the TerminalPane scroll + output + input state machine.
 * Mirrors the real code paths for typing while scrolled up.
 */
class InputScrollSimulator {
  scrollbackOffset = 0;
  totalScrollback = 0;
  isUserScrolled = false;
  scrollSeq = 0;
  diffSeq = 0;
  snapshotPending = false;

  cachedSnapshot: {
    scrollback_offset: number;
    total_scrollback: number;
    rows: Array<{ cells: string[]; wrapped: boolean }>;
  } | null = null;

  /** Mirror of TerminalPane.handleScroll (line 489) */
  handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    ++this.scrollSeq;
    this.cachedSnapshot = null;
    mockSetScrollback(newOffset);
  }

  /** Mirror of TerminalPane.snapToBottom (line 523) */
  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    ++this.scrollSeq;
    this.cachedSnapshot = null;
    mockSetScrollback(0);
    // In real code: terminalService.setScrollback(0).then(fetchFullSnapshot)
    // The fetch is async — deferred until IPC resolves
  }

  /** Simulate a full snapshot arriving from the daemon */
  applySnapshot(offset: number, total: number) {
    this.cachedSnapshot = {
      scrollback_offset: offset,
      total_scrollback: total,
      rows: [{ cells: ['prompt> typed text'], wrapped: false }],
    };
    if (!this.isUserScrolled) {
      this.scrollbackOffset = offset;
    } else if (offset > this.scrollbackOffset) {
      this.scrollbackOffset = offset;
    }
    this.totalScrollback = total;
    mockRender(this.cachedSnapshot);
  }

  /**
   * Mirror of the grid diff event handler (TerminalPane.ts line 222-234).
   * Called when a pushed diff arrives from the daemon after PTY output.
   *
   * @param echoContent - The echoed text in the diff (represents what the user typed)
   * @param diffOffset - The scrollback_offset in the diff (0 = live view)
   */
  onPushedDiff(diffOffset: number, total: number, echoContent?: string) {
    // Bug #238: The diff handler (line 228) checks autoScrollOnOutput
    if (this.isUserScrolled && terminalSettingsStore.getAutoScrollOnOutput()) {
      this.snapToBottom();
      return; // <-- Returns WITHOUT applying the diff
    }

    // Apply diff to cache (mirrors applyPushedDiff)
    if (!this.cachedSnapshot) {
      mockFetchSnapshot();
      return;
    }

    this.cachedSnapshot.scrollback_offset = diffOffset;
    this.cachedSnapshot.total_scrollback = total;
    if (echoContent) {
      this.cachedSnapshot.rows = [{ cells: [echoContent], wrapped: false }];
    }

    // Sync offset from daemon
    if (!this.isUserScrolled) {
      this.scrollbackOffset = diffOffset;
    } else if (diffOffset > this.scrollbackOffset) {
      this.scrollbackOffset = diffOffset;
    }
    this.totalScrollback = total;

    this.diffSeq++;
    mockRender(this.cachedSnapshot);
  }

  /**
   * Mirror of the terminal output event handler (TerminalPane.ts line 239-253).
   * Fallback pull path when no pushed diff is available.
   */
  onTerminalOutput() {
    if (this.isUserScrolled && terminalSettingsStore.getAutoScrollOnOutput()) {
      this.snapToBottom();
      return; // <-- Returns WITHOUT scheduling snapshot fetch
    }
    mockFetchSnapshot();
  }

  /**
   * Simulate user typing a character.
   * In real code: textarea input event → snapToBottom() → writeToTerminal().
   * Fix #238: snap to live view before sending data to PTY so the echo is visible.
   */
  onUserInput(_text: string) {
    // Fix #238: snap to live view when user sends input to PTY.
    // No-op if already at bottom (scrollbackOffset === 0 guard in snapToBottom).
    this.snapToBottom();
  }

  /** Whether the viewport is showing the live view (offset 0) */
  get isAtLiveView(): boolean {
    return this.scrollbackOffset === 0;
  }

  /** Whether the renderer was called with content showing the echo */
  get lastRenderedContent(): string | null {
    if (mockRender.mock.calls.length === 0) return null;
    const lastCall = mockRender.mock.calls[mockRender.mock.calls.length - 1];
    const snapshot = lastCall[0];
    return snapshot?.rows?.[0]?.cells?.[0] ?? null;
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #238: typing while scrolled up should update display', () => {
  let sim: InputScrollSimulator;

  beforeEach(() => {
    sim = new InputScrollSimulator();
    mockSetScrollback.mockClear();
    mockFetchSnapshot.mockClear();
    mockRender.mockClear();
  });

  afterEach(() => {
    terminalSettingsStore.setAutoScrollOnOutput(false);
  });

  // ── Core bug: viewport doesn't snap on user input ──────────────────

  describe('user input should snap viewport to live view', () => {
    it('Bug #238: typing while scrolled up should snap to live view (default settings)', () => {
      // Default: autoScrollOnOutput = false
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      // User has scrolled up 5 lines
      sim.applySnapshot(0, 100);
      sim.handleScroll(5);
      expect(sim.isUserScrolled).toBe(true);
      expect(sim.scrollbackOffset).toBe(5);

      // User types a character
      sim.onUserInput('a');

      // Bug #238: After typing, the viewport should snap to live view
      // so the user can see the echo of what they typed.
      // Current behavior: viewport stays at offset 5, echo is invisible.
      expect(sim.isAtLiveView).toBe(true);
    });

    it('Bug #238: typing while scrolled up by 1 line should snap to live view', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.applySnapshot(0, 50);
      sim.handleScroll(1); // Scroll up by just 1 line
      expect(sim.scrollbackOffset).toBe(1);

      sim.onUserInput('x');

      // Even 1 line of scroll should snap back on user input
      expect(sim.isAtLiveView).toBe(true);
    });

    it('Bug #238: typing while deeply scrolled should snap to live view', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.applySnapshot(0, 500);
      sim.handleScroll(200); // Scroll up 200 lines
      expect(sim.scrollbackOffset).toBe(200);

      sim.onUserInput('hello');

      expect(sim.isAtLiveView).toBe(true);
    });
  });

  // ── Echo visibility after typing ────────────────────────────────────

  describe('echo should be visible after typing', () => {
    it('Bug #238: echo diff after snap triggers fetch (cache was cleared)', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.applySnapshot(0, 100);
      sim.handleScroll(10);

      // User types — snap clears cache
      sim.onUserInput('a');
      expect(sim.isAtLiveView).toBe(true);

      // PTY echoes — diff arrives but cache is null (cleared by snap),
      // so it falls back to fetch path. The fetch will return live view
      // content (including the echo) since viewport is now at offset 0.
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 100, 'prompt> a');
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });

    it('Bug #238: echo via terminal output event should snap to live view before fetch', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.applySnapshot(0, 100);
      sim.handleScroll(5);

      sim.onUserInput('b');
      mockFetchSnapshot.mockClear();
      sim.onTerminalOutput();

      // After user input, the viewport is at live view and a fetch is triggered.
      // The fetch returns content at offset 0 (live view), which includes the echo.
      expect(sim.isAtLiveView).toBe(true);
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });

    it('Bug #238: multiple characters typed while scrolled — all trigger fetches', () => {
      expect(terminalSettingsStore.getAutoScrollOnOutput()).toBe(false);

      sim.applySnapshot(0, 100);
      sim.handleScroll(3);

      // First character: snap clears cache, diff falls back to fetch
      sim.onUserInput('h');
      expect(sim.isAtLiveView).toBe(true);
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 100, 'prompt> h');
      expect(mockFetchSnapshot).toHaveBeenCalledTimes(1);

      // Second character: already at bottom (snap is no-op), diff still
      // has no cache (fetch hasn't resolved yet), triggers another fetch
      sim.onUserInput('i');
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 100, 'prompt> hi');
      expect(mockFetchSnapshot).toHaveBeenCalledTimes(1);
      expect(sim.isAtLiveView).toBe(true);
    });
  });

  // ── autoScrollOnOutput=true path ────────────────────────────────────

  describe('autoScrollOnOutput enabled: user input snap prevents double-snap', () => {
    it('Fix #238: with autoScrollOnOutput=true, diff handler does NOT double-snap', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);

      sim.applySnapshot(0, 100);
      sim.handleScroll(5);
      expect(sim.isUserScrolled).toBe(true);

      // User types — snaps to bottom (isUserScrolled becomes false)
      sim.onUserInput('a');
      expect(sim.isAtLiveView).toBe(true);
      expect(sim.isUserScrolled).toBe(false);

      // Echo diff arrives. Since isUserScrolled is already false,
      // the autoScrollOnOutput guard is NOT triggered (no double-snap).
      // Cache is null (cleared by snap), so falls back to fetch path.
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 100, 'prompt> a');

      // The diff handler proceeds normally (no early return) and triggers fetch
      expect(sim.isAtLiveView).toBe(true);
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });

    it('Fix #238: autoScrollOnOutput guard skipped after user input snap', () => {
      terminalSettingsStore.setAutoScrollOnOutput(true);

      sim.applySnapshot(0, 100);
      sim.handleScroll(10);

      // User types — snaps to bottom before write
      sim.onUserInput('x');
      expect(sim.isAtLiveView).toBe(true);

      // Diff arrives — isUserScrolled is false, so the guard
      // (isUserScrolled && autoScrollOnOutput) is NOT triggered.
      // Proceeds to normal diff/fetch path instead of snap+return.
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 100, 'prompt> x');

      expect(sim.isAtLiveView).toBe(true);
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });
  });

  // ── Claude Code usage pattern ──────────────────────────────────────

  describe('Claude Code usage pattern', () => {
    it('Fix #238: user reviews output, scrolls up slightly, types in prompt — viewport snaps', () => {
      // 1. Claude Code produces output, user is at live view
      sim.applySnapshot(0, 200);
      expect(sim.isAtLiveView).toBe(true);

      // 2. User scrolls up 3 lines to review something
      sim.handleScroll(3);
      expect(sim.scrollbackOffset).toBe(3);
      expect(sim.isUserScrolled).toBe(true);

      // 3. User starts typing in Claude Code's prompt area — snaps to bottom
      sim.onUserInput('tell me about');
      expect(sim.isAtLiveView).toBe(true);

      // 4. The echo arrives as a diff — cache is null, triggers fetch
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 200, 'prompt> tell me about');

      // 5. Fetch is triggered — will return live view with echo
      expect(mockFetchSnapshot).toHaveBeenCalled();
      expect(sim.isAtLiveView).toBe(true);
    });

    it('Bug #238: sustained output while user types — echo lost in scrollback', () => {
      // 1. Initial state with some output
      sim.applySnapshot(0, 100);

      // 2. More output arrives, scrollback grows
      for (let i = 1; i <= 20; i++) {
        sim.onPushedDiff(0, 100 + i);
      }

      // 3. User scrolls up to review
      sim.handleScroll(5);
      expect(sim.isUserScrolled).toBe(true);

      // 4. User types while still scrolled
      sim.onUserInput('fix the bug');

      // 5. Should be back at live view
      expect(sim.isAtLiveView).toBe(true);
    });

    it('Fix #238: scroll position not exactly at end — offset=1 no longer blocks', () => {
      // The bug title says "scroll position isn't exactly at the end"
      // Even 1 line of scroll offset should snap back on user input
      sim.applySnapshot(0, 50);
      sim.handleScroll(1); // Just 1 line!

      sim.onUserInput('a');

      // The viewport must snap back to show the echo
      expect(sim.scrollbackOffset).toBe(0);
      expect(sim.isUserScrolled).toBe(false);

      // Echo diff triggers fetch (cache cleared by snap)
      mockFetchSnapshot.mockClear();
      sim.onPushedDiff(0, 50, 'a');
      expect(mockFetchSnapshot).toHaveBeenCalled();
    });
  });
});
