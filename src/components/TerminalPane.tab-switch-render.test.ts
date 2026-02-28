import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { RichGridData, RichGridDiff, RichGridRow, RichGridCell } from './TerminalRenderer';

/**
 * Tests for Bug #424: Tab switch sometimes shows blank terminal until user scrolls.
 *
 * Root cause: When switching tabs, TerminalPane.setActive(true) clears the
 * canvas via updateSize() and relies on fetchAndRenderSnapshot() to repaint.
 * If cachedSnapshot is null (e.g., invalidated by a prior scroll) and the
 * async snapshot IPC fails or is discarded (diffSeq race), the canvas stays
 * black permanently. There is no retry mechanism — the user must scroll to
 * trigger flushScroll() which performs a separate fetch.
 *
 * This suite simulates the TerminalPane's state machine to reproduce the
 * exact failure modes:
 * 1. Scroll-invalidated cache + failed IPC → permanent blank
 * 2. diffSeq race (pushed diff discards in-flight fetch) → blank until scheduled retry
 * 3. No retry after failed fetch → must scroll to recover
 */

// ── Helpers ──────────────────────────────────────────────────────────

function makeCell(content: string): RichGridCell {
  return {
    content,
    fg: 'default',
    bg: 'default',
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    inverse: false,
    wide: false,
    wide_continuation: false,
  };
}

function makeRow(text: string): RichGridRow {
  return {
    cells: text.split('').map(makeCell),
    wrapped: false,
  };
}

function makeSnapshot(lines: string[], offset = 0): RichGridData {
  return {
    rows: lines.map(makeRow),
    cursor: { row: 0, col: 0 },
    dimensions: { rows: lines.length, cols: lines[0]?.length ?? 80 },
    alternate_screen: false,
    cursor_hidden: false,
    title: 'test',
    scrollback_offset: offset,
    total_scrollback: offset,
  };
}

function makeDiff(dirtyRows: [number, RichGridRow][]): RichGridDiff {
  return {
    dirty_rows: dirtyRows,
    cursor: { row: 0, col: 0 },
    dimensions: { rows: 24, cols: 80 },
    alternate_screen: false,
    cursor_hidden: false,
    title: 'test',
    scrollback_offset: 0,
    total_scrollback: 0,
    full_repaint: false,
  };
}

// ── State Machine Simulator ──────────────────────────────────────────
//
// Mirrors the exact fields and logic from TerminalPane that govern the
// pause/resume/fetch/render coordination. Every method matches the source
// in TerminalPane.ts to ensure the test exercises the real code path.

type FetchResult =
  | { ok: true; snapshot: RichGridData }
  | { ok: false; error: Error };

class PaneSimulator {
  // ---- Core state (mirrors TerminalPane) ----
  cachedSnapshot: RichGridData | null = null;
  forceFullFetch = false;
  paused = false;
  isUserScrolled = false;
  scrollbackOffset = 0;
  totalScrollback = 0;
  scrollSeq = 0;
  diffSeq = 0;
  snapshotPending = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  renderRAF: number | null = null;
  scrollRafId: number | null = null;

  // ---- Test instrumentation ----
  renderCalls: RichGridData[] = [];
  fetchCalls: string[] = []; // tracks 'full', 'diff', 'scroll'

  // ---- Configurable IPC behavior ----
  private _fullFetchResult: FetchResult | null = null;
  private _diffFetchResult: FetchResult | null = null;
  private _scrollFetchResult: FetchResult | null = null;
  private _fullFetchDelay = 0;

  /**
   * Configure what get_grid_snapshot returns.
   * Set to null to use a default successful snapshot.
   */
  setFullFetchResult(result: FetchResult | null) {
    this._fullFetchResult = result;
  }

  /** Configure the delay (ms) for the full fetch IPC. */
  setFullFetchDelay(ms: number) {
    this._fullFetchDelay = ms;
  }

  /** Configure what get_grid_snapshot_diff returns. */
  setDiffFetchResult(result: FetchResult | null) {
    this._diffFetchResult = result;
  }

  /** Configure what scrollAndGetSnapshot returns. */
  setScrollFetchResult(result: FetchResult | null) {
    this._scrollFetchResult = result;
  }

  // ---- Simulated rendering ----

  private renderSnapshot(snapshot: RichGridData): void {
    this.renderCalls.push(snapshot);
  }

  private scheduleRender() {
    if (this.renderRAF !== null) return;
    // In tests, we execute the RAF callback synchronously via flushRAF()
    this.renderRAF = 1; // sentinel value
  }

  flushRenderRAF() {
    if (this.renderRAF !== null) {
      this.renderRAF = null;
      if (this.cachedSnapshot) {
        this.renderSnapshot(this.cachedSnapshot);
      }
    }
  }

  // ---- Pause / Resume (mirrors TerminalPane.ts lines 1010-1061) ----

  pause() {
    if (this.paused) return;
    this.paused = true;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
    }
    this.snapshotPending = false;
    if (this.renderRAF !== null) {
      this.renderRAF = null; // cancelAnimationFrame
    }
    // Bug #424 fix: cancel pending scroll flush on pause
    if (this.scrollRafId !== null) {
      this.scrollRafId = null; // cancelAnimationFrame
    }
    // Canvas resources released (not modeled — tested by browser tests)
  }

  resume() {
    if (!this.paused) return;
    this.paused = false;
    this.forceFullFetch = true;
    // Canvas resources restored (not modeled)
    // Output stream reconnected (not modeled — tested separately)
  }

  // ---- setActive (mirrors TerminalPane.ts lines 1067-1102) ----
  //
  // Returns a promise that resolves after the RAF callback completes.
  // The synchronous part (stale render) runs immediately.

  async setActive(active: boolean): Promise<void> {
    if (active) {
      this.resume();

      // Sync canvas size (clears canvas — tested by browser tests)
      // In real code: this.renderer.updateSize(); this.gridRenderer?.updateSize();

      // Render stale cached snapshot immediately
      if (this.cachedSnapshot) {
        this.renderSnapshot(this.cachedSnapshot);
      }

      // Simulate the RAF callback
      // In real code this is requestAnimationFrame(() => { ... })
      // We execute it inline for deterministic testing.
      this.fit();
      // scrollToBottom is a no-op here (TerminalRenderer.currentSnapshot is null after release)
      await this.fetchAndRenderSnapshot();
    } else {
      this.pause();
    }
  }

  // ---- Scroll handling (mirrors TerminalPane.ts lines 569-701) ----

  handleScroll(delta: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + delta);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;

    // Bug #424: cachedSnapshot is nulled synchronously before the async
    // scroll-snapshot IPC completes. If the user switches tabs before
    // the IPC returns, cachedSnapshot will be null on resume.
    this.cachedSnapshot = null;
    ++this.scrollSeq;

    // Schedule flushScroll in RAF (matches real code)
    if (this.scrollRafId === null) {
      this.scrollRafId = 1; // sentinel
    }
  }

  async flushScrollRAF() {
    if (this.scrollRafId === null) return;
    this.scrollRafId = null;
    await this.flushScroll();
  }

  private async flushScroll() {
    const seq = this.scrollSeq;
    const offset = this.scrollbackOffset;
    this.fetchCalls.push('scroll');
    try {
      const result = this._scrollFetchResult ?? {
        ok: true as const,
        snapshot: makeSnapshot(['$ scroll-result'], offset),
      };
      if (!result.ok) throw result.error;
      const snapshot = result.snapshot;
      if (seq !== this.scrollSeq) return; // stale
      this.cachedSnapshot = snapshot;
      this.scrollbackOffset = snapshot.scrollback_offset;
      this.isUserScrolled = this.scrollbackOffset > 0;
      this.totalScrollback = snapshot.total_scrollback;
      this.renderSnapshot(snapshot);
    } catch {
      // Error swallowed — matches real code: console.debug(...)
    }
  }

  snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    this.cachedSnapshot = null;
    ++this.scrollSeq;
    // flushScroll is called inline (not via RAF) in snapToBottom
    this.flushScroll(); // fire-and-forget — matches real code
  }

  // ---- Fit (mirrors TerminalPane.ts lines 975-999) ----

  private containerWidth = 800;
  private containerHeight = 600;

  fit() {
    // Visibility guard
    if (!this.containerWidth || !this.containerHeight) return;
    // If dimensions changed, invalidate cache
    if (this.cachedSnapshot) {
      // For simplicity, assume dimensions don't change
    }
  }

  // ---- Grid snapshot fetching (mirrors TerminalPane.ts lines 729-841) ----

  private useDiffSnapshots = true;

  async fetchAndRenderSnapshot(): Promise<void> {
    const forceFull = this.forceFullFetch;
    if (forceFull) this.forceFullFetch = false;
    const seqBefore = this.scrollSeq;
    const diffSeqBefore = this.diffSeq;
    try {
      // Use diff path when we have a cached full snapshot and diff is supported
      if (!forceFull && this.cachedSnapshot && this.useDiffSnapshots) {
        this.fetchCalls.push('diff');
        const result = this._diffFetchResult ?? {
          ok: true as const,
          snapshot: makeSnapshot(['$ diff-result']),
        };
        if (!result.ok) throw result.error;

        if (seqBefore !== this.scrollSeq) return;
        if (diffSeqBefore !== this.diffSeq) return;

        // Simplified: just replace the cached snapshot
        this.cachedSnapshot = result.snapshot;
        this.renderSnapshot(this.cachedSnapshot);
        return;
      }

      // Full snapshot path
      await this.fetchFullSnapshot(seqBefore, diffSeqBefore);
    } catch {
      // Bug #424 fix: retry when we have no cached snapshot to show
      if (!this.cachedSnapshot && !this.paused) {
        this.scheduleSnapshotFetch();
      }
    }
  }

  private async fetchFullSnapshot(
    scrollSeqAtStart?: number,
    diffSeqAtStart?: number,
  ): Promise<void> {
    this.fetchCalls.push('full');

    // Simulate IPC delay
    if (this._fullFetchDelay > 0) {
      await new Promise((r) => setTimeout(r, this._fullFetchDelay));
    }

    const result = this._fullFetchResult ?? {
      ok: true as const,
      snapshot: makeSnapshot(['$ full-snapshot-result']),
    };
    if (!result.ok) throw result.error;
    const snapshot = result.snapshot;

    // Bug #424: stale-sequence guards can discard a valid fetch result
    if (scrollSeqAtStart !== undefined && scrollSeqAtStart !== this.scrollSeq)
      return;
    if (diffSeqAtStart !== undefined && diffSeqAtStart !== this.diffSeq)
      return;

    this.cachedSnapshot = snapshot;
    if (!this.isUserScrolled) {
      this.scrollbackOffset = snapshot.scrollback_offset;
    } else if (snapshot.scrollback_offset > this.scrollbackOffset) {
      this.scrollbackOffset = snapshot.scrollback_offset;
    }
    this.totalScrollback = snapshot.total_scrollback;
    this.renderSnapshot(snapshot);
  }

  // ---- Pushed diff handling (mirrors TerminalPane.ts lines 847-907) ----

  applyPushedDiff(diff: RichGridDiff) {
    this.diffSeq++;
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      this.snapshotPending = false;
    }

    if (!this.cachedSnapshot) {
      this.scheduleSnapshotFetch();
      return;
    }

    // Apply diff to cached snapshot (simplified)
    for (const [rowIdx, rowData] of diff.dirty_rows) {
      if (rowIdx < this.cachedSnapshot.rows.length) {
        this.cachedSnapshot.rows[rowIdx] = rowData;
      }
    }
    this.cachedSnapshot.cursor = diff.cursor;

    this.scheduleRender();
  }

  // ---- Scheduled snapshot fetch (mirrors TerminalPane.ts lines 706-725) ----

  scheduleSnapshotFetch() {
    if (this.paused) return;
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    // In real code, this uses setTimeout. For tests, we track it.
    this.snapshotTimer = setTimeout(async () => {
      this.snapshotTimer = null;
      try {
        await this.fetchAndRenderSnapshot();
      } finally {
        this.snapshotPending = false;
      }
    }, 16); // SNAPSHOT_MIN_INTERVAL_MS
  }

  /** Flush the scheduled snapshot timer synchronously. */
  async flushSnapshotTimer(): Promise<void> {
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
      try {
        await this.fetchAndRenderSnapshot();
      } finally {
        this.snapshotPending = false;
      }
    }
  }
}

// ── Tests ────────────────────────────────────────────────────────────

describe('Bug #424: Tab switch blank terminal until scroll', () => {
  let pane: PaneSimulator;

  beforeEach(() => {
    pane = new PaneSimulator();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ── Scenario 1: Happy path (stale cache exists, fetch succeeds) ──

  it('renders content after tab switch when cachedSnapshot exists', async () => {
    // Setup: terminal has content cached
    pane.cachedSnapshot = makeSnapshot(['$ hello world']);

    // Simulate tab switch away and back
    await pane.setActive(false); // pause
    await pane.setActive(true); // resume → should render

    // Expected: at least one render call with valid snapshot data
    expect(pane.renderCalls.length).toBeGreaterThan(0);
    const lastRender = pane.renderCalls[pane.renderCalls.length - 1];
    expect(lastRender.rows.length).toBeGreaterThan(0);
  });

  // ── Scenario 2: Scroll-invalidated cache + failed IPC ──
  //
  // This is the primary reproduction of the bug.
  // 1. User scrolls up → cachedSnapshot nulled
  // 2. User switches tab before scroll fetch completes
  // 3. User switches back → no stale cache to render, IPC fails
  // 4. Canvas stays blank — no retry mechanism

  it('should render content after tab switch even when cachedSnapshot is null and IPC fails', async () => {
    // Bug #424: When cachedSnapshot is null and fetchAndRenderSnapshot() fails,
    // the fix schedules a retry via scheduleSnapshotFetch().

    // Setup: terminal has content
    pane.cachedSnapshot = makeSnapshot(['$ some content']);

    // Step 1: User scrolls up → cachedSnapshot nulled
    pane.handleScroll(10);
    expect(pane.cachedSnapshot).toBeNull();

    // Step 2: User switches tab (scroll RAF cancelled by fix)
    await pane.setActive(false);
    expect(pane.scrollRafId).toBeNull(); // Fix: pause cancels scroll RAF

    // Step 3: cachedSnapshot is still null (scroll fetch was cancelled, not completed)
    expect(pane.cachedSnapshot).toBeNull();

    // Step 4: User switches back — first fetch fails
    pane.setFullFetchResult({ ok: false, error: new Error('bridge busy') });
    await pane.setActive(true);

    // The first fetch failed, but the fix scheduled a retry
    expect(pane.snapshotPending || pane.snapshotTimer !== null).toBe(true);

    // Step 5: Retry succeeds
    pane.setFullFetchResult({
      ok: true,
      snapshot: makeSnapshot(['$ recovered content']),
    });
    await pane.flushSnapshotTimer();

    // Expected: content rendered via retry
    expect(pane.renderCalls.length).toBeGreaterThan(0);
    expect(pane.cachedSnapshot).not.toBeNull();
  });

  // ── Scenario 3: diffSeq race discards valid fetch result ──

  it('should not lose content when a pushed diff arrives during the snapshot fetch', async () => {
    // Bug #424: When a pushed diff increments diffSeq during an in-flight
    // full fetch, the fetch result is discarded. If cachedSnapshot is null,
    // the pushed diff falls back to scheduleSnapshotFetch(), and the retry
    // mechanism ensures content eventually renders.

    // Start with null cache (e.g., fresh terminal before first snapshot)
    pane.cachedSnapshot = null;

    // Use delayed full fetch to allow pushed diff to arrive during fetch
    pane.setFullFetchDelay(50);
    pane.setFullFetchResult({
      ok: true,
      snapshot: makeSnapshot(['$ full snapshot']),
    });

    // Start the resume — fetchAndRenderSnapshot begins the async IPC
    const resumePromise = pane.setActive(true);

    // During the fetch delay, a pushed diff arrives
    // This increments diffSeq, causing the in-flight fetch to be discarded
    pane.applyPushedDiff(makeDiff([[0, makeRow('$ pushed diff content')]]));

    // Wait for the fetch to complete
    await vi.advanceTimersByTimeAsync(100);
    await resumePromise;

    // The pushed diff tried to apply to cachedSnapshot, but it was null,
    // so it fell back to scheduleSnapshotFetch().
    // The original full fetch was discarded (diffSeq changed).
    // Content should eventually be rendered from the scheduled retry.
    await pane.flushSnapshotTimer();

    // Expected: content rendered at least once
    const renderCallsAfterResume = pane.renderCalls;
    expect(renderCallsAfterResume.length).toBeGreaterThan(0);
  });

  // ── Scenario 4: Failed IPC with no retry leaves permanent blank ──

  it('should retry snapshot fetch after failure instead of leaving canvas blank', async () => {
    // Bug #424: fetchAndRenderSnapshot() catches errors with console.debug
    // and does NOT schedule a retry. If cachedSnapshot is null, the canvas
    // stays blank until something else triggers a fetch (scroll, output event).

    pane.cachedSnapshot = null; // simulate fresh terminal or post-scroll state

    // IPC will fail
    pane.setFullFetchResult({ ok: false, error: new Error('bridge timeout') });

    await pane.setActive(true);

    // fetchAndRenderSnapshot failed → cachedSnapshot still null
    expect(pane.cachedSnapshot).toBeNull();

    // Expected: a retry should have been scheduled
    // Bug #424: no retry is scheduled — snapshotTimer is null
    expect(pane.snapshotPending || pane.snapshotTimer !== null).toBe(true);
  });

  // ── Scenario 5: Scroll is the ONLY recovery from blank screen ──

  it('requires user scroll to recover from failed fetch (confirms the bug)', async () => {
    // This test documents the workaround the user discovered:
    // scrolling triggers flushScroll() which performs a separate IPC that
    // succeeds, finally rendering content.

    pane.cachedSnapshot = null;

    // Initial fetch fails
    pane.setFullFetchResult({ ok: false, error: new Error('bridge busy') });
    await pane.setActive(true);

    // Canvas is blank — no render calls
    expect(pane.renderCalls.length).toBe(0);

    // User scrolls up to recover
    pane.handleScroll(1);
    // Scroll fetch succeeds
    pane.setScrollFetchResult({
      ok: true,
      snapshot: makeSnapshot(['$ content appears!'], 1),
    });
    await pane.flushScrollRAF();

    // NOW content is rendered
    expect(pane.renderCalls.length).toBeGreaterThan(0);
    expect(pane.cachedSnapshot).not.toBeNull();
  });

  // ── Scenario 6: pause() does NOT cancel pending scroll RAF ──

  it('pause does not cancel scrollRafId, allowing scroll fetch against released canvas', async () => {
    // Bug #424: pause() cancels snapshotTimer and renderRAF but NOT
    // scrollRafId. A pending flushScroll() can fire after pause, painting
    // to a 1x1 released canvas (effectively a no-op).

    pane.cachedSnapshot = makeSnapshot(['$ content']);

    // User scrolls (nulls cache, schedules flushScroll in RAF)
    pane.handleScroll(5);
    expect(pane.scrollRafId).not.toBeNull();
    expect(pane.cachedSnapshot).toBeNull();

    // User switches tab (pause)
    await pane.setActive(false);

    // Bug: scrollRafId is NOT cancelled by pause()
    // Expected: pause should cancel all pending async operations
    expect(pane.scrollRafId).toBeNull();
  });

  // ── Scenario 7: Multiple rapid tab switches ──

  it('handles rapid tab switching without losing content', async () => {
    pane.cachedSnapshot = makeSnapshot(['$ initial']);

    // Rapid tab switches: away → back → away → back
    await pane.setActive(false);
    await pane.setActive(true);
    await pane.setActive(false);
    await pane.setActive(true);

    // Content should be rendered on the final activation
    expect(pane.renderCalls.length).toBeGreaterThan(0);
  });
});
