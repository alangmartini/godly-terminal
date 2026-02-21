import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Bug #218: Typing rollback — stale pulled snapshot overwrites fresher pushed diff.
 *
 * When typing in Godly Terminal, characters briefly appear then disappear
 * before reappearing. Root cause: the frontend has two independent rendering
 * paths (pushed diffs and pulled snapshots) with no staleness guard.
 * A pulled snapshot initiated before an echo can complete after a pushed diff
 * that contains the echo, overwriting the correct state with stale data.
 *
 * The fix adds a `diffSeq` monotonic counter (same pattern as scrollSeq):
 * - Incremented in applyPushedDiff()
 * - Checked after IPC in fetchAndRenderSnapshot() / fetchFullSnapshot()
 * - Also cancels any queued snapshot timer in applyPushedDiff()
 *
 * Run: npx vitest run src/components/TerminalPane.typing-rollback.test.ts
 */

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * Minimal simulator that mirrors the TerminalPane state machine for the
 * diff/snapshot race condition. Tracks diffSeq, scrollSeq, snapshot timer,
 * and rendering calls without mocking Tauri IPC.
 */
class TypingRollbackSimulator {
  // Core state
  diffSeq = 0;
  scrollSeq = 0;
  snapshotPending = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  cachedSnapshot: { cursor_row: number; text: string } | null = null;

  // Track renders for assertions
  renderLog: Array<{ source: 'diff' | 'pulled-diff' | 'pulled-full'; text: string }> = [];

  // Track discards for assertions
  discardLog: Array<{ reason: string; text: string }> = [];

  /** Mirror of TerminalPane.applyPushedDiff */
  applyPushedDiff(text: string) {
    // Bug #218 fix: increment diffSeq and cancel pending timer
    this.diffSeq++;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      this.snapshotPending = false;
    }

    this.cachedSnapshot = { cursor_row: 0, text };
    this.renderLog.push({ source: 'diff', text });
  }

  /** Mirror of TerminalPane.scheduleSnapshotFetch */
  scheduleSnapshotFetch() {
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    this.snapshotTimer = setTimeout(() => {
      this.snapshotTimer = null;
      // Timer fired — the IPC call would happen here.
      // In tests we simulate the IPC response via completePulledSnapshot().
    }, 16);
  }

  /**
   * Simulate a pulled diff snapshot IPC response arriving.
   * Mirrors fetchAndRenderSnapshot() diff path with the diffSeq guard.
   */
  completePulledDiffSnapshot(diffSeqBefore: number, scrollSeqBefore: number, text: string) {
    // Guard: scroll changed
    if (scrollSeqBefore !== this.scrollSeq) {
      this.discardLog.push({ reason: 'scroll-stale', text });
      return;
    }
    // Bug #218 guard: diff arrived since fetch started
    if (diffSeqBefore !== this.diffSeq) {
      this.discardLog.push({ reason: 'diff-stale', text });
      return;
    }

    this.cachedSnapshot = { cursor_row: 0, text };
    this.renderLog.push({ source: 'pulled-diff', text });
    this.snapshotPending = false;
  }

  /**
   * Simulate a pulled full snapshot IPC response arriving.
   * Mirrors fetchFullSnapshot() with optional diffSeqAtStart guard.
   */
  completePulledFullSnapshot(
    scrollSeqBefore: number | undefined,
    diffSeqBefore: number | undefined,
    text: string,
  ) {
    if (scrollSeqBefore !== undefined && scrollSeqBefore !== this.scrollSeq) {
      this.discardLog.push({ reason: 'scroll-stale', text });
      return;
    }
    // Bug #218 guard
    if (diffSeqBefore !== undefined && diffSeqBefore !== this.diffSeq) {
      this.discardLog.push({ reason: 'diff-stale', text });
      return;
    }

    this.cachedSnapshot = { cursor_row: 0, text };
    this.renderLog.push({ source: 'pulled-full', text });
    this.snapshotPending = false;
  }

  /** Capture current diffSeq (mirrors `const diffSeqBefore = this.diffSeq` before IPC) */
  captureDiffSeq(): number {
    return this.diffSeq;
  }

  /** Capture current scrollSeq */
  captureScrollSeq(): number {
    return this.scrollSeq;
  }

  /** Mirror of scroll-triggered fetch (doesn't pass diffSeqAtStart) */
  handleScrollFetch(text: string, scrollSeqBefore: number) {
    this.completePulledFullSnapshot(scrollSeqBefore, undefined, text);
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #218: typing rollback — diffSeq staleness guard', () => {
  let sim: TypingRollbackSimulator;

  beforeEach(() => {
    sim = new TypingRollbackSimulator();
    vi.useFakeTimers();
  });

  // ── Core race condition ────────────────────────────────────────────────

  describe('pulled snapshot after diff is discarded', () => {
    it('Bug #218: stale pulled diff-snapshot is discarded when diff arrived during fetch', () => {
      // T0: Output event triggers snapshot fetch
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      // T1: IPC request in flight... (simulated by holding diffSeqBefore/scrollSeqBefore)

      // T3: Echo arrives as pushed diff — renders 'a' correctly
      sim.applyPushedDiff('$ a');
      expect(sim.renderLog).toHaveLength(1);
      expect(sim.renderLog[0]).toEqual({ source: 'diff', text: '$ a' });

      // T4: Stale snapshot IPC response arrives (captured before echo)
      sim.completePulledDiffSnapshot(diffSeqBefore, scrollSeqBefore, '$ ');

      // Assert: stale snapshot was discarded, not rendered
      expect(sim.renderLog).toHaveLength(1); // still just the diff render
      expect(sim.discardLog).toHaveLength(1);
      expect(sim.discardLog[0].reason).toBe('diff-stale');

      // Assert: cached snapshot still has the correct (fresher) state
      expect(sim.cachedSnapshot!.text).toBe('$ a');
    });

    it('Bug #218: stale pulled full-snapshot is discarded when diff arrived during fetch', () => {
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      // Diff arrives while full snapshot fetch is in flight
      sim.applyPushedDiff('$ ab');

      // Full snapshot arrives with stale data
      sim.completePulledFullSnapshot(scrollSeqBefore, diffSeqBefore, '$ ');

      expect(sim.renderLog).toHaveLength(1);
      expect(sim.renderLog[0].source).toBe('diff');
      expect(sim.discardLog).toHaveLength(1);
      expect(sim.discardLog[0].reason).toBe('diff-stale');
      expect(sim.cachedSnapshot!.text).toBe('$ ab');
    });

    it('Bug #218: multiple diffs during single fetch all increment diffSeq', () => {
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      // Two rapid diffs while snapshot is in flight
      sim.applyPushedDiff('$ a');
      sim.applyPushedDiff('$ ab');

      // Stale snapshot arrives
      sim.completePulledDiffSnapshot(diffSeqBefore, scrollSeqBefore, '$ ');

      expect(sim.diffSeq).toBe(2);
      expect(sim.renderLog).toHaveLength(2); // two diffs rendered
      expect(sim.discardLog).toHaveLength(1); // one stale snapshot discarded
      expect(sim.cachedSnapshot!.text).toBe('$ ab');
    });
  });

  // ── No false positives ─────────────────────────────────────────────────

  describe('pulled snapshot without intervening diff is applied', () => {
    it('snapshot without diff is rendered normally (diffSeq unchanged)', () => {
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      // No diff arrives during fetch
      sim.completePulledDiffSnapshot(diffSeqBefore, scrollSeqBefore, '$ ls');

      expect(sim.renderLog).toHaveLength(1);
      expect(sim.renderLog[0]).toEqual({ source: 'pulled-diff', text: '$ ls' });
      expect(sim.discardLog).toHaveLength(0);
    });

    it('full snapshot without diff is rendered normally', () => {
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      sim.completePulledFullSnapshot(scrollSeqBefore, diffSeqBefore, '$ pwd');

      expect(sim.renderLog).toHaveLength(1);
      expect(sim.renderLog[0]).toEqual({ source: 'pulled-full', text: '$ pwd' });
      expect(sim.discardLog).toHaveLength(0);
    });

    it('snapshot after diff has settled is applied (diffSeq matches)', () => {
      // Diff arrives and renders
      sim.applyPushedDiff('$ a');
      expect(sim.renderLog).toHaveLength(1);

      // New snapshot fetch starts AFTER the diff — captures current diffSeq
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();

      // Snapshot arrives — diffSeq matches, so it's applied
      sim.completePulledDiffSnapshot(diffSeqBefore, scrollSeqBefore, '$ a');

      expect(sim.renderLog).toHaveLength(2);
      expect(sim.renderLog[1].source).toBe('pulled-diff');
      expect(sim.discardLog).toHaveLength(0);
    });
  });

  // ── Timer cancellation ─────────────────────────────────────────────────

  describe('timer cancellation on diff arrival', () => {
    it('Bug #218: pending snapshot timer is canceled when diff arrives', () => {
      // Output event schedules a snapshot fetch
      sim.scheduleSnapshotFetch();
      expect(sim.snapshotPending).toBe(true);
      expect(sim.snapshotTimer).not.toBeNull();

      // Diff arrives before timer fires — should cancel the timer
      sim.applyPushedDiff('$ x');

      expect(sim.snapshotPending).toBe(false);
      expect(sim.snapshotTimer).toBeNull();
    });

    it('Bug #218: timer does not fire after diff cancels it', () => {
      sim.scheduleSnapshotFetch();
      sim.applyPushedDiff('$ y');

      // Advance past the 16ms timer interval
      vi.advanceTimersByTime(50);

      // Timer should not have fired (snapshotPending stays false)
      expect(sim.snapshotPending).toBe(false);
      expect(sim.snapshotTimer).toBeNull();
    });

    it('new snapshot can be scheduled after diff cancellation', () => {
      sim.scheduleSnapshotFetch();
      sim.applyPushedDiff('$ z');

      // After cancellation, a new output event can schedule a fresh fetch
      sim.scheduleSnapshotFetch();
      expect(sim.snapshotPending).toBe(true);
      expect(sim.snapshotTimer).not.toBeNull();
    });
  });

  // ── Scroll-triggered fetches unaffected ────────────────────────────────

  describe('scroll-triggered fetches are unaffected by diffSeq', () => {
    it('scroll-triggered fetch is applied even when diffSeq changed', () => {
      // Diff arrives
      sim.applyPushedDiff('$ a');

      // Scroll-triggered fetch (doesn't pass diffSeqAtStart)
      const scrollSeqBefore = sim.captureScrollSeq();
      sim.handleScrollFetch('scrolled view content', scrollSeqBefore);

      // Should be applied because scroll fetches don't check diffSeq
      expect(sim.renderLog).toHaveLength(2);
      expect(sim.renderLog[1].source).toBe('pulled-full');
      expect(sim.discardLog).toHaveLength(0);
      expect(sim.cachedSnapshot!.text).toBe('scrolled view content');
    });

    it('scroll-triggered fetch still checks scrollSeq', () => {
      // Capture scroll seq, then scroll again (incrementing scrollSeq)
      const scrollSeqBefore = sim.captureScrollSeq();
      sim.scrollSeq++; // simulate another scroll happening

      sim.handleScrollFetch('stale scroll content', scrollSeqBefore);

      // Should be discarded due to scrollSeq mismatch
      expect(sim.renderLog).toHaveLength(0);
      expect(sim.discardLog).toHaveLength(1);
      expect(sim.discardLog[0].reason).toBe('scroll-stale');
    });
  });

  // ── Full race timeline ─────────────────────────────────────────────────

  describe('full race timeline (Bug #218 repro)', () => {
    it('reproduces the exact T0-T5 race from the bug report', () => {
      // T0: Output event → scheduleSnapshotFetch (16ms timer)
      sim.scheduleSnapshotFetch();

      // T1: Timer fires → IPC request sent (snapshot fetch in flight)
      vi.advanceTimersByTime(16);
      const diffSeqBefore = sim.captureDiffSeq();
      const scrollSeqBefore = sim.captureScrollSeq();
      // (IPC is now in flight)

      // T2: User types 'a' → fire-and-forget write (no sim action needed)

      // T3: Echo arrives → diff pushed → renders 'a' correctly
      sim.applyPushedDiff('$ a');
      expect(sim.cachedSnapshot!.text).toBe('$ a');
      expect(sim.renderLog[0]).toEqual({ source: 'diff', text: '$ a' });

      // T4: Snapshot IPC response arrives (captured before echo) → MUST be discarded
      sim.completePulledDiffSnapshot(diffSeqBefore, scrollSeqBefore, '$ ');

      // Assert: 'a' was NOT overwritten — the stale snapshot was discarded
      expect(sim.cachedSnapshot!.text).toBe('$ a');
      expect(sim.renderLog).toHaveLength(1); // only the diff render
      expect(sim.discardLog).toHaveLength(1);
      expect(sim.discardLog[0]).toEqual({ reason: 'diff-stale', text: '$ ' });

      // T5: Next diff → 'a' still visible (no rollback)
      sim.applyPushedDiff('$ ab');
      expect(sim.cachedSnapshot!.text).toBe('$ ab');
      expect(sim.renderLog).toHaveLength(2);
    });
  });
});
