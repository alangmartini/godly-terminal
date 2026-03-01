import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Bug #486: Terminal text completely invisible until tab switch.
 *
 * All terminal text is invisible after mount. Only the cursor blink is
 * visible (if the initial snapshot populated it). Switching to another
 * tab and back makes text appear. Root cause: a race condition between
 * the binary diff stream and the initial snapshot fetch creates a
 * deadlock where cachedSnapshot stays null permanently.
 *
 * Two trigger paths:
 * 1. Mount race: diff arrives during initial fetchAndRenderSnapshot() IPC
 *    → snapshot discarded by stale diffSeq check → cachedSnapshot stays
 *    null → subsequent diffs call scheduleSnapshotFetch() → blocked by
 *    diffStreamActive guard → deadlock.
 * 2. Resize race: fit() nulls cachedSnapshot while diffStreamActive is
 *    true → same deadlock as above.
 *
 * These tests assert correct behavior after the fix — the terminal must
 * always recover from race conditions and render content.
 *
 * Run: npx vitest run src/components/TerminalPane.diff-stream-deadlock.test.ts
 */

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * Minimal simulator that mirrors the TerminalPane state machine for the
 * diff-stream deadlock. Models the interaction between:
 * - diffStreamActive flag (suppresses the pull path)
 * - cachedSnapshot (null until first successful fetch)
 * - diffSeq / scrollSeq (staleness guards)
 * - forceFullFetch (bypass for diffStreamActive guard)
 * - paused flag (pause/resume lifecycle)
 *
 * This mirrors the FIXED code. Tests assert that cachedSnapshot
 * gets populated — which it does thanks to the forceFullFetch fix.
 */
class DiffStreamDeadlockSimulator {
  // Core state (mirrors TerminalPane private fields)
  diffStreamActive = false;
  forceFullFetch = false;
  cachedSnapshot: { text: string } | null = null;
  diffSeq = 0;
  scrollSeq = 0;
  snapshotPending = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  paused = false;

  // Track renders for assertions
  renderLog: Array<{ source: 'diff' | 'full-fetch'; text: string }> = [];

  // Track discards for assertions
  discardLog: Array<{ reason: string; text: string }> = [];

  // Pending IPC context (simulates an in-flight fetchAndRenderSnapshot call)
  pendingFetch: {
    diffSeqBefore: number;
    scrollSeqBefore: number;
  } | null = null;

  /**
   * Mirror of TerminalPane.scheduleSnapshotFetch()
   * Line 723-744 in TerminalPane.ts
   */
  scheduleSnapshotFetch() {
    if (this.paused) return;
    // Bug #486: This guard blocks fetching when cachedSnapshot is null
    if (this.diffStreamActive && !this.forceFullFetch) return;
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    this.snapshotTimer = setTimeout(() => {
      this.snapshotTimer = null;
    }, 16);
  }

  /**
   * Mirror of TerminalPane.applyPushedDiff()
   * Line 872-932 in TerminalPane.ts
   */
  applyPushedDiff(text: string) {
    this.diffSeq++;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      this.snapshotPending = false;
    }

    if (!this.cachedSnapshot) {
      // Bug #486 fix: Set forceFullFetch so scheduleSnapshotFetch()
      // bypasses the diffStreamActive guard.
      this.forceFullFetch = true;
      this.scheduleSnapshotFetch();
      return;
    }

    // Merge diff into cached snapshot
    this.cachedSnapshot = { text };
    this.renderLog.push({ source: 'diff', text });
  }

  /**
   * Start fetchAndRenderSnapshot() — captures seq values for staleness check.
   * Mirrors line 749-833 in TerminalPane.ts
   */
  startFetch(): void {
    const forceFull = this.forceFullFetch;
    if (forceFull) this.forceFullFetch = false;

    this.pendingFetch = {
      diffSeqBefore: this.diffSeq,
      scrollSeqBefore: this.scrollSeq,
    };
  }

  /**
   * Complete the IPC call — applies staleness checks.
   * Mirrors fetchFullSnapshot() line 844-866 in TerminalPane.ts
   */
  completeFetch(text: string): boolean {
    if (!this.pendingFetch) throw new Error('No pending fetch');

    const { diffSeqBefore, scrollSeqBefore } = this.pendingFetch;
    this.pendingFetch = null;

    if (scrollSeqBefore !== this.scrollSeq) {
      this.discardLog.push({ reason: 'scroll-stale', text });
      return false;
    }

    // Bug #218 guard: if a diff arrived during fetch, discard as stale
    if (diffSeqBefore !== this.diffSeq) {
      this.discardLog.push({ reason: 'diff-stale', text });
      return false;
    }

    this.cachedSnapshot = { text };
    this.renderLog.push({ source: 'full-fetch', text });
    this.snapshotPending = false;
    return true;
  }

  /** Mirror of pause() — line 1035-1061 */
  pause() {
    if (this.paused) return;
    this.paused = true;
    this.diffStreamActive = false;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
    }
    this.snapshotPending = false;
  }

  /** Mirror of resume() — line 1073-1107 */
  resume() {
    if (!this.paused) return;
    this.paused = false;
    this.forceFullFetch = true;
  }

  /** Mirror of fit() — line 1000-1020, nulls cachedSnapshot on dimension change */
  fit(dimensionsChanged: boolean = true) {
    if (dimensionsChanged && this.cachedSnapshot) {
      this.cachedSnapshot = null;
      // Bug #486 fix: Ensure the next scheduleSnapshotFetch() bypasses the
      // diffStreamActive guard so a resize doesn't permanently blank the terminal.
      this.forceFullFetch = true;
    }
  }

  /**
   * Simulate the diff stream callback (connectDiffStream handler).
   * Sets diffStreamActive THEN calls applyPushedDiff.
   * Mirrors line 344-353 in TerminalPane.ts
   */
  onDiffStreamData(text: string) {
    this.diffStreamActive = true;
    this.applyPushedDiff(text);
  }

  /**
   * Helper: simulate the scheduled fetch firing and completing.
   * In real code, the timer fires → fetchAndRenderSnapshot() → IPC → render.
   * This advances fake timers, then runs startFetch()+completeFetch().
   */
  runScheduledFetch(text: string): boolean {
    if (!this.snapshotPending) return false;
    vi.advanceTimersByTime(20); // fire the scheduled timer
    this.startFetch();
    return this.completeFetch(text);
  }
}

// ── Tests ────────────────────────────────────────────────────────────────
// These tests verify that the forceFullFetch fix breaks the deadlock:
// recovery fetches are scheduled and complete successfully.

describe('Bug #486: diff stream deadlock — terminal text invisible until tab switch', () => {
  let sim: DiffStreamDeadlockSimulator;

  beforeEach(() => {
    sim = new DiffStreamDeadlockSimulator();
    vi.useFakeTimers();
  });

  // ── Mount race condition ─────────────────────────────────────────────

  describe('mount race: diff arrives during initial snapshot fetch', () => {
    it('Bug #486: terminal must render text after diff arrives during initial fetch', () => {
      // Simulate mount():
      // 1. connectDiffStream (async connection)
      // 2. RAF → fetchAndRenderSnapshot (starts IPC)
      sim.startFetch();

      // Binary diff stream delivers first frame BEFORE IPC resolves.
      // (Shell prompt arrived fast — common with PowerShell on Windows.)
      sim.onDiffStreamData('PS C:\\> ');

      // Initial snapshot IPC resolves — discarded because diffSeq changed
      sim.completeFetch('PS C:\\> ');

      // Fix: forceFullFetch was set, so a recovery fetch was scheduled.
      // Verify the deadlock is broken: snapshotPending must be true.
      expect(sim.snapshotPending).toBe(true);

      // Simulate the scheduled recovery fetch completing
      sim.runScheduledFetch('PS C:\\> ');

      // EXPECTED: cachedSnapshot is populated with the shell prompt.
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ');
    });

    it('Bug #486: subsequent diffs must render after mount race', () => {
      // Enter the race condition scenario
      sim.startFetch();
      sim.onDiffStreamData('PS C:\\> ');
      sim.completeFetch('PS C:\\> '); // discarded

      // Recovery fetch completes — breaks the deadlock
      sim.runScheduledFetch('PS C:\\> ');
      expect(sim.cachedSnapshot).not.toBeNull();

      // More diffs arrive — user types, shell produces output
      sim.onDiffStreamData('PS C:\\> dir');
      sim.onDiffStreamData('PS C:\\> dir\nfoo.txt\nbar.txt');

      // EXPECTED: the last diff's content is rendered
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> dir\nfoo.txt\nbar.txt');
      expect(sim.renderLog.length).toBeGreaterThan(0);
    });

    it('Bug #486: multiple rapid diffs during initial fetch must not create permanent blank', () => {
      sim.startFetch();

      // 3 rapid diffs before IPC resolves
      sim.onDiffStreamData('PS C:\\> ');
      sim.onDiffStreamData('PS C:\\> d');
      sim.onDiffStreamData('PS C:\\> di');

      // Stale snapshot arrives
      sim.completeFetch('old state');

      // Fix: forceFullFetch ensures a recovery fetch is scheduled
      expect(sim.snapshotPending).toBe(true);

      // Recovery fetch completes with latest state
      sim.runScheduledFetch('PS C:\\> di');

      // EXPECTED: terminal shows the latest content
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> di');
    });
  });

  // ── Resize race condition ──────────────────────────────────────────

  describe('resize race: fit() nulls cachedSnapshot while diffStreamActive', () => {
    it('Bug #486: diffs must render after resize invalidates cache', () => {
      // Normal startup: initial fetch succeeds, diffs work fine
      sim.startFetch();
      sim.completeFetch('PS C:\\> ');
      sim.onDiffStreamData('PS C:\\> ls');
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ls');

      // Window resize triggers fit() — dimensions change → cache nulled
      sim.fit(/* dimensionsChanged = */ true);
      expect(sim.cachedSnapshot).toBeNull();
      // Fix: fit() sets forceFullFetch when nulling cache
      expect(sim.forceFullFetch).toBe(true);

      // Next diff arrives — cachedSnapshot still null, so recovery fetch scheduled
      sim.onDiffStreamData('PS C:\\> ls\nfoo.txt');

      // Recovery fetch completes
      sim.runScheduledFetch('PS C:\\> ls\nfoo.txt');

      // EXPECTED: terminal shows the new content after resize
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ls\nfoo.txt');
    });

    it('Bug #486: maximize should not permanently blank the terminal', () => {
      // Successful init
      sim.startFetch();
      sim.completeFetch('$ ');
      sim.onDiffStreamData('$ hello');
      expect(sim.cachedSnapshot!.text).toBe('$ hello');

      // User clicks maximize — ResizeObserver fires, fit() called
      sim.fit(true);

      // 10 rapid diffs while cache is null — each sets forceFullFetch
      for (let i = 0; i < 10; i++) {
        sim.onDiffStreamData(`line ${i}`);
      }

      // Fix: a recovery fetch was scheduled; simulate it completing
      sim.runScheduledFetch('line 9');

      // EXPECTED: terminal shows the latest content
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('line 9');
    });
  });

  // ── Recovery verification ──────────────────────────────────────────

  describe('tab switch recovery (proves the deadlock is real)', () => {
    it('Bug #486: fix prevents deadlock so tab switch is not needed for recovery', () => {
      // Enter the race condition (same as mount race test)
      sim.startFetch();
      sim.onDiffStreamData('PS C:\\> ');
      sim.completeFetch('PS C:\\> '); // discarded (stale diffSeq)

      // Fix: forceFullFetch was set, recovery fetch scheduled
      expect(sim.snapshotPending).toBe(true);

      // Recovery fetch completes — no tab switch needed
      sim.runScheduledFetch('PS C:\\> ');

      // With the fix, cachedSnapshot is NOT null after recovery
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ');
    });
  });

  // ── Normal operation (no race) — these should PASS ─────────────────

  describe('no deadlock when fetch completes before first diff (control group)', () => {
    it('normal startup: fetch succeeds → diffs render', () => {
      sim.startFetch();
      sim.completeFetch('PS C:\\> ');
      expect(sim.cachedSnapshot).not.toBeNull();

      sim.onDiffStreamData('PS C:\\> dir');
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> dir');
      expect(sim.renderLog).toHaveLength(2);
    });

    it('fit() without dimension change preserves cache', () => {
      sim.startFetch();
      sim.completeFetch('$ ');
      sim.onDiffStreamData('$ hello');

      sim.fit(/* dimensionsChanged = */ false);
      expect(sim.cachedSnapshot).not.toBeNull();

      sim.onDiffStreamData('$ hello world');
      expect(sim.cachedSnapshot!.text).toBe('$ hello world');
    });
  });
});
