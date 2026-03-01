import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Bug #486 (iteration 3): Recovery fetch livelock under sustained diff traffic.
 *
 * Previous fixes (PR #489 + commit 0180cbf) broke the initial deadlock and
 * added in-flight retry queuing. But the underlying livelock remains:
 *
 * 1. cachedSnapshot is null → recovery fetch starts → captures diffSeqBefore
 * 2. During the async IPC roundtrip, a diff arrives → diffSeq++
 * 3. IPC completes → diffSeqBefore !== diffSeq → snapshot DISCARDED
 * 4. applyPushedDiff() re-arms forceFullFetch (cache still null) → retry
 * 5. Retry captures new diffSeqBefore → another diff arrives → DISCARDED again
 * 6. Loop forever — cachedSnapshot never gets populated
 *
 * Root cause: fetchFullSnapshot() always checks diffSeqAtStart (Bug #218
 * typing rollback prevention), even on recovery fetches where cachedSnapshot
 * is null and there's nothing to "roll back" to.
 *
 * These tests verify the fix: recovery fetches (cachedSnapshot is null)
 * skip the diffSeq staleness check, so the snapshot is accepted even when
 * diffs arrive during the IPC roundtrip.
 *
 * Run: npx vitest run src/components/TerminalPane.recovery-livelock.test.ts
 */

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * High-fidelity simulator of TerminalPane's snapshot/diff state machine.
 * Models the async IPC gap where diffs arrive between fetch-start and
 * fetch-complete — the window the previous tests missed.
 *
 * Mirrors the FIXED code: recovery fetches (cachedSnapshot null at start)
 * skip the diffSeq staleness check so they always accept the snapshot.
 */
class RecoveryLivelockSimulator {
  // Core state (mirrors TerminalPane private fields)
  diffStreamActive = false;
  forceFullFetch = false;
  cachedSnapshot: { text: string } | null = null;
  diffSeq = 0;
  scrollSeq = 0;
  snapshotPending = false;
  snapshotRetryRequested = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  renderRAF: number | null = null;
  paused = false;

  // In-flight fetch state (simulates the async IPC gap)
  pendingFetch: {
    diffSeqBefore: number;
    scrollSeqBefore: number;
    forceFull: boolean;
    // Track whether cachedSnapshot was null when the fetch started
    // (this is the missing information the current code doesn't track)
    wasRecovery: boolean;
  } | null = null;

  // Observability
  renderCount = 0;
  discardCount = 0;
  fetchAttemptCount = 0;

  /**
   * Mirror of TerminalPane.scheduleSnapshotFetch() (line 726-760).
   */
  scheduleSnapshotFetch() {
    if (this.paused) return;
    if (this.diffStreamActive && !this.forceFullFetch) return;
    if (this.snapshotPending) {
      if (this.forceFullFetch) {
        this.snapshotRetryRequested = true;
      }
      return;
    }
    this.snapshotPending = true;
    this.snapshotTimer = setTimeout(() => {
      this.snapshotTimer = null;
      // Timer fired — start the fetch
      this.startFetchAndRender();
    }, 16);
  }

  /**
   * Mirror of TerminalPane.fetchAndRenderSnapshot() (line 764-851).
   * Captures state, sets up the pending fetch for async completion.
   */
  private startFetchAndRender() {
    const forceFull = this.forceFullFetch;
    if (forceFull) this.forceFullFetch = false;

    const diffSeqBefore = this.diffSeq;
    const scrollSeqBefore = this.scrollSeq;

    // In the real code, if !forceFull && cachedSnapshot && useDiffSnapshots,
    // it takes the diff-snapshot path. For recovery (cachedSnapshot is null
    // or forceFull is true), it falls through to fetchFullSnapshot.
    // We simulate the full-fetch path since that's what recovery uses.
    this.pendingFetch = {
      diffSeqBefore,
      scrollSeqBefore,
      forceFull,
      wasRecovery: this.cachedSnapshot === null,
    };
    this.fetchAttemptCount++;
  }

  /**
   * Mirror of TerminalPane.applyPushedDiff() (line 890-953).
   */
  applyPushedDiff(text: string) {
    // Bug #218: Increment diffSeq to invalidate in-flight pulled snapshots
    this.diffSeq++;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      this.snapshotPending = false;
    }

    if (!this.cachedSnapshot) {
      // Bug #486 fix (PR #489): Set forceFullFetch to bypass diffStreamActive guard
      this.forceFullFetch = true;
      this.scheduleSnapshotFetch();
      return;
    }

    // Merge diff into cached snapshot
    this.cachedSnapshot = { text };
    this.renderCount++;
  }

  /**
   * Mirror of the diff stream callback (line 347-356).
   */
  onDiffStreamData(text: string) {
    this.diffStreamActive = true;
    this.applyPushedDiff(text);
  }

  /**
   * Complete the in-flight IPC call. Mirrors fetchFullSnapshot() (line 862-884).
   * Bug #486 fix: recovery fetches (wasRecovery=true) skip the diffSeq
   * staleness check since there's nothing to "roll back" to.
   */
  completePendingFetch(text: string): boolean {
    if (!this.pendingFetch) throw new Error('No pending fetch');

    const { diffSeqBefore, scrollSeqBefore, wasRecovery } = this.pendingFetch;
    this.pendingFetch = null;

    // Scroll staleness check
    if (scrollSeqBefore !== this.scrollSeq) {
      this.discardCount++;
      this.finishScheduledFetch();
      return false;
    }

    // Bug #218 staleness check — prevents typing rollback where a pre-echo
    // snapshot overwrites a post-echo diff.
    // Bug #486 fix: Skip this check for recovery fetches (cachedSnapshot was
    // null at fetch start). Recovery fetches have nothing to roll back — they
    // NEED the snapshot to escape the livelock.
    if (!wasRecovery && diffSeqBefore !== this.diffSeq) {
      this.discardCount++;
      this.finishScheduledFetch();
      return false;
    }

    this.cachedSnapshot = { text };
    this.renderCount++;
    this.finishScheduledFetch();
    return true;
  }

  /**
   * Run the finally block from scheduleSnapshotFetch's timer callback.
   */
  private finishScheduledFetch() {
    this.snapshotPending = false;
    if (this.snapshotRetryRequested && !this.paused) {
      this.snapshotRetryRequested = false;
      this.scheduleSnapshotFetch();
    }
  }

  /**
   * Simulate one complete recovery cycle:
   * 1. Advance timers to fire the scheduled fetch
   * 2. Deliver a diff during the IPC gap (simulating typing echo or shell output)
   * 3. Complete the IPC with the server's response
   *
   * Returns true if the snapshot was accepted.
   */
  runRecoveryCycleWithInterferingDiff(
    diffText: string,
    fetchResultText: string,
  ): boolean {
    // Fire the scheduled timer → starts the fetch
    vi.advanceTimersByTime(20);
    if (!this.pendingFetch) return false;

    // Diff arrives during the async IPC roundtrip
    this.onDiffStreamData(diffText);

    // IPC completes
    return this.completePendingFetch(fetchResultText);
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #486 (iteration 3): recovery fetch livelock under sustained diff traffic', () => {
  let sim: RecoveryLivelockSimulator;

  beforeEach(() => {
    sim = new RecoveryLivelockSimulator();
    vi.useFakeTimers();
  });

  describe('fix: recovery fetches skip diffSeq staleness check', () => {
    it('recovery fetch accepted despite diff arriving during IPC', () => {
      // Bug #486: initial fetch is a recovery (cachedSnapshot null).
      // A diff arrives during the IPC roundtrip, incrementing diffSeq.
      // Fix: recovery fetches skip the diffSeq check, so the snapshot
      // is accepted and the terminal displays content.
      sim.pendingFetch = {
        diffSeqBefore: 0,
        scrollSeqBefore: 0,
        forceFull: false,
        wasRecovery: true,
      };
      sim.fetchAttemptCount = 1;

      // Diff arrives during IPC → diffSeq increments
      sim.onDiffStreamData('PS C:\\> ');

      // IPC completes — wasRecovery=true so diffSeq check is skipped
      const accepted = sim.completePendingFetch('PS C:\\> ');
      expect(accepted).toBe(true);
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ');
    });

    it('sustained typing echo does not prevent recovery (livelock fixed)', () => {
      // Bug #486 livelock: under sustained diffs, recovery never succeeded.
      // Fix: first recovery attempt accepts the snapshot regardless of diffSeq.
      sim.diffStreamActive = true;

      // Initial diff with no cache → recovery scheduled
      sim.onDiffStreamData('PS C:\\> ');
      expect(sim.snapshotPending).toBe(true);

      // First recovery attempt with interfering diff — should succeed immediately
      const accepted = sim.runRecoveryCycleWithInterferingDiff(
        'PS C:\\> a',
        'PS C:\\> a',
      );

      expect(accepted).toBe(true);
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> a');
      expect(sim.discardCount).toBe(0);
    });

    it('user typing echo does not block initial display', () => {
      // Realistic scenario: user types immediately, shell echoes each keystroke.
      // Fix: the first recovery fetch succeeds despite typing echo diffs.

      // Mount: initial fetch starts (recovery — cachedSnapshot is null)
      sim.pendingFetch = {
        diffSeqBefore: 0,
        scrollSeqBefore: 0,
        forceFull: false,
        wasRecovery: true,
      };
      sim.fetchAttemptCount = 1;

      // Shell prompt arrives as diff during initial fetch
      sim.onDiffStreamData('PS C:\\> ');

      // IPC returns — recovery, so diffSeq check skipped → accepted
      const accepted = sim.completePendingFetch('PS C:\\> ');
      expect(accepted).toBe(true);
      expect(sim.cachedSnapshot).not.toBeNull();

      // Subsequent diffs merge normally (cachedSnapshot is now populated)
      sim.onDiffStreamData('PS C:\\> hello');
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> hello');
      expect(sim.renderCount).toBeGreaterThanOrEqual(2);
    });
  });

  describe('Bug #218 protection preserved for non-recovery fetches', () => {
    it('non-recovery fetch is still discarded when diffSeq changes', () => {
      // When cachedSnapshot EXISTS, the diffSeq staleness check must still
      // apply to prevent Bug #218 typing rollback.
      sim.cachedSnapshot = { text: 'PS C:\\> hello' };
      sim.diffStreamActive = true;
      sim.renderCount = 1;

      // Start a non-recovery fetch (cachedSnapshot is NOT null)
      sim.pendingFetch = {
        diffSeqBefore: sim.diffSeq,
        scrollSeqBefore: sim.scrollSeq,
        forceFull: false,
        wasRecovery: false, // NOT a recovery — cachedSnapshot exists
      };
      sim.fetchAttemptCount = 1;

      // Diff arrives during IPC (typing echo with newer content)
      sim.onDiffStreamData('PS C:\\> hello world');

      // IPC returns with OLDER state — must be discarded (Bug #218)
      const accepted = sim.completePendingFetch('PS C:\\> hel');
      expect(accepted).toBe(false);
      expect(sim.discardCount).toBe(1);

      // cachedSnapshot retains the NEWER state from the diff
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> hello world');
    });
  });

  describe('recovery succeeds without tab switch (no workaround needed)', () => {
    it('recovery with interfering diffs resolves inline without tab switch', () => {
      // Before the fix, this scenario required switching tabs to recover.
      // After the fix, recovery succeeds on the first attempt.
      sim.diffStreamActive = true;
      sim.onDiffStreamData('PS C:\\> ');

      // Recovery attempt with diff interference
      const accepted = sim.runRecoveryCycleWithInterferingDiff(
        'PS C:\\> a',
        'PS C:\\> a',
      );

      // Fix: recovery succeeds immediately — no tab switch needed
      expect(accepted).toBe(true);
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> a');
    });
  });

  describe('control group: recovery without interference', () => {
    it('recovery fetch succeeds when no diff arrives during IPC', () => {
      sim.diffStreamActive = true;
      sim.onDiffStreamData('PS C:\\> ');
      expect(sim.snapshotPending).toBe(true);

      vi.advanceTimersByTime(20);
      expect(sim.pendingFetch).not.toBeNull();

      const accepted = sim.completePendingFetch('PS C:\\> ');
      expect(accepted).toBe(true);
      expect(sim.cachedSnapshot).not.toBeNull();
      expect(sim.cachedSnapshot!.text).toBe('PS C:\\> ');
    });
  });
});
