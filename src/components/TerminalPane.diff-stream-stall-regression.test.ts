import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Bug #486 regression variant:
 * typing/output updates only appear after switching terminals away and back.
 *
 * Trigger:
 * 1. A diff frame arrives once -> `diffStreamActive = true`.
 * 2. Diff stream stalls/disconnects while tab remains active (no pause()).
 * 3. Output fallback callbacks fire, but TerminalPane ignores them because
 *    `diffStreamActive` is still true.
 *
 * Expected:
 * Fallback output should schedule a snapshot fetch without requiring a tab switch.
 *
 * Run: npx vitest run src/components/TerminalPane.diff-stream-stall-regression.test.ts
 */

class DiffStreamStallSimulator {
  paused = false;
  diffStreamActive = false;
  forceFullFetch = false;
  snapshotPending = false;
  diffStallFallbackTimer: ReturnType<typeof setTimeout> | null = null;

  static readonly DIFF_STALL_FALLBACK_MS = 250;

  outputEventsHandled = 0;

  /**
   * Mirror of TerminalPane.scheduleSnapshotFetch() guard logic.
   */
  scheduleSnapshotFetch() {
    if (this.paused) return;
    if (this.diffStreamActive && !this.forceFullFetch) return;
    if (this.snapshotPending) return;
    this.snapshotPending = true;
  }

  /**
   * Mirror of connectOutputStream callback in TerminalPane.mount()/resume().
   */
  onOutputStreamData() {
    if (this.paused) return;
    if (this.diffStreamActive) {
      this.armDiffStallFallback();
      return;
    }
    this.outputEventsHandled++;
    this.scheduleSnapshotFetch();
  }

  /**
   * Mirror of onTerminalOutput event handler in TerminalPane.mount().
   */
  onTerminalOutputEvent() {
    if (this.paused) return;
    if (this.diffStreamActive) {
      this.armDiffStallFallback();
      return;
    }
    this.outputEventsHandled++;
    this.scheduleSnapshotFetch();
  }

  /**
   * Mirror of connectDiffStream callback path: latch diff mode.
   */
  onDiffFrame() {
    if (this.paused) return;
    this.clearDiffStallFallback();
    this.diffStreamActive = true;
  }

  /**
   * Current production behavior: no callback clears diffStreamActive when the
   * diff stream silently stalls. This no-op documents that missing transition.
   */
  onDiffStreamStalled() {
    // Intentionally empty.
  }

  armDiffStallFallback() {
    if (this.diffStallFallbackTimer !== null) return;
    this.diffStallFallbackTimer = setTimeout(() => {
      this.diffStallFallbackTimer = null;
      if (this.paused) return;
      if (!this.diffStreamActive) return;
      this.diffStreamActive = false;
      this.forceFullFetch = true;
      this.scheduleSnapshotFetch();
    }, DiffStreamStallSimulator.DIFF_STALL_FALLBACK_MS);
  }

  clearDiffStallFallback() {
    if (this.diffStallFallbackTimer !== null) {
      clearTimeout(this.diffStallFallbackTimer);
      this.diffStallFallbackTimer = null;
    }
  }

  /**
   * Mirror of TerminalPane.pause(): tab switch away.
   */
  pause() {
    this.paused = true;
    this.diffStreamActive = false;
    this.snapshotPending = false;
    this.clearDiffStallFallback();
  }

  /**
   * Mirror of TerminalPane.resume(): tab switch back.
   */
  resume() {
    this.paused = false;
    this.forceFullFetch = true;
  }
}

describe('Bug #486 regression: diff stream stall suppresses in-tab updates', () => {
  let sim: DiffStreamStallSimulator;

  beforeEach(() => {
    sim = new DiffStreamStallSimulator();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should recover via fallback output without requiring tab switch', () => {
    // Diff stream is healthy at first and latches diff mode.
    sim.onDiffFrame();
    expect(sim.diffStreamActive).toBe(true);

    // Stream then stalls while this tab remains active.
    sim.onDiffStreamStalled();

    // User types; shell emits output notifications.
    sim.onOutputStreamData();
    sim.onTerminalOutputEvent();

    // Expected: fallback path should schedule a recovery snapshot in-place.
    // The fallback is timer-driven: if no diff frame arrives in the grace
    // window, diff mode is dropped and a recovery snapshot is scheduled.
    expect(sim.snapshotPending).toBe(false);
    vi.advanceTimersByTime(DiffStreamStallSimulator.DIFF_STALL_FALLBACK_MS + 1);
    expect(sim.snapshotPending).toBe(true);
  });

  it('tab switch away/back resets diff mode and allows recovery scheduling', () => {
    sim.onDiffFrame();
    sim.onDiffStreamStalled();

    // User performs the workaround.
    sim.pause();
    sim.resume();

    // Next output callback can now drive recovery.
    sim.onOutputStreamData();
    expect(sim.snapshotPending).toBe(true);
    expect(sim.outputEventsHandled).toBe(1);
  });
});
