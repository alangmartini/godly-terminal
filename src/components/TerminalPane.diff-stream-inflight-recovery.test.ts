import { beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Bug #486 regression: screen can stay stale/blank after typing.
 *
 * Trigger:
 * 1. `cachedSnapshot` is null and a recovery fetch is already in flight.
 * 2. A new pushed diff (typed echo) arrives before that fetch resolves.
 * 3. The in-flight fetch is discarded by `diffSeq` staleness, but no follow-up
 *    recovery fetch is queued because `snapshotPending` blocked re-scheduling.
 *
 * Expected:
 * A second recovery fetch must be queued so the screen updates.
 *
 * Run: npx vitest run src/components/TerminalPane.diff-stream-inflight-recovery.test.ts
 */

class InFlightRecoveryRaceSimulator {
  diffStreamActive = true;
  forceFullFetch = false;
  cachedSnapshot: { text: string } | null = null;
  diffSeq = 0;
  scrollSeq = 0;
  snapshotPending = false;
  snapshotRetryRequested = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  inFlight:
    | {
        diffSeqBefore: number;
        scrollSeqBefore: number;
        fromScheduled: boolean;
      }
    | null = null;

  /**
   * Mirrors TerminalPane.scheduleSnapshotFetch().
   */
  scheduleSnapshotFetch() {
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
      const forceFull = this.forceFullFetch;
      if (forceFull) this.forceFullFetch = false;
      this.inFlight = {
        diffSeqBefore: this.diffSeq,
        scrollSeqBefore: this.scrollSeq,
        fromScheduled: true,
      };
    }, 16);
  }

  /**
   * Mirrors the subset of TerminalPane.applyPushedDiff() used in this race.
   */
  onPushedDiff(_text: string) {
    this.diffSeq++;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      this.snapshotPending = false;
    }

    if (!this.cachedSnapshot) {
      this.forceFullFetch = true;
      this.scheduleSnapshotFetch();
      return;
    }
  }

  /**
   * Completes the in-flight scheduled fetch and applies staleness guards.
   */
  completeInFlightFetch(text: string): boolean {
    if (!this.inFlight) throw new Error('No in-flight fetch');
    const { diffSeqBefore, scrollSeqBefore, fromScheduled } = this.inFlight;
    this.inFlight = null;

    const isStale = diffSeqBefore !== this.diffSeq || scrollSeqBefore !== this.scrollSeq;
    if (!isStale) {
      this.cachedSnapshot = { text };
    }

    // Mirrors the timer callback's `finally` in scheduleSnapshotFetch().
    if (fromScheduled) {
      this.snapshotPending = false;
      if (this.snapshotRetryRequested) {
        this.snapshotRetryRequested = false;
        this.scheduleSnapshotFetch();
      }
    }

    return !isStale;
  }
}

describe('Bug #486 regression: in-flight recovery fetch vs typing diff', () => {
  let sim: InFlightRecoveryRaceSimulator;

  beforeEach(() => {
    sim = new InFlightRecoveryRaceSimulator();
    vi.useFakeTimers();
  });

  it('should queue a second recovery fetch when a diff arrives during an in-flight scheduled fetch', () => {
    // First diff arrives with no cache: schedule recovery fetch (timer path).
    sim.onPushedDiff('PS C:\\> ');
    expect(sim.snapshotPending).toBe(true);

    // Timer fires -> scheduled fetch starts (snapshotPending remains true until completion).
    vi.advanceTimersByTime(20);
    expect(sim.inFlight).not.toBeNull();
    expect(sim.snapshotPending).toBe(true);

    // Bug trigger (#486 regression): typed echo arrives while fetch is in flight.
    // No cache exists yet, so this requests another recovery fetch.
    sim.onPushedDiff('PS C:\\> a');

    // In-flight fetch returns stale data and is discarded by diffSeq guard.
    const applied = sim.completeInFlightFetch('PS C:\\> ');
    expect(applied).toBe(false);
    expect(sim.cachedSnapshot).toBeNull();

    // Expected correct behavior: a follow-up recovery fetch is queued.
    // Pre-fix behavior: snapshotPending stayed false here, leaving the
    // screen stale/blank until some later output happened.
    expect(sim.snapshotPending).toBe(true);
  });
});
