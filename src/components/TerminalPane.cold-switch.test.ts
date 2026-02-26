import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Tests for Bug #373: Slow terminal content load on tab switch (2-3s black screen).
 *
 * When switching to a terminal that hasn't been focused for a while, the screen
 * shows black for 2-3 seconds before content appears. The root cause is that
 * resume() nulls the cached snapshot before the async full-fetch completes,
 * leaving the screen blank during the IPC round-trip.
 *
 * The fix should implement a "stale-while-revalidate" pattern: render the stale
 * cached snapshot immediately on resume, then replace it with fresh data when
 * the background fetch completes.
 */

// ── Mocks ────────────────────────────────────────────────────────────────

interface MockSnapshot {
  rows: Array<{ cells: unknown[]; wrapped: boolean }>;
  cursor: { row: number; col: number };
  dimensions: { rows: number; cols: number };
  alternate_screen: boolean;
  cursor_hidden: boolean;
  title: string;
  scrollback_offset: number;
  total_scrollback: number;
}

function createMockSnapshot(label: string): MockSnapshot {
  return {
    rows: [{ cells: [{ char: label }], wrapped: false }],
    cursor: { row: 0, col: 0 },
    dimensions: { rows: 24, cols: 80 },
    alternate_screen: false,
    cursor_hidden: false,
    title: `terminal-${label}`,
    scrollback_offset: 0,
    total_scrollback: 100,
  };
}

const mockDisconnectOutputStream = vi.fn();
const mockConnectOutputStream = vi.fn();
const mockTriggerProbe = vi.fn();
const mockRenderSnapshot = vi.fn();

// Simulate IPC fetch that resolves after a delay (like real bridge round-trip)
let fetchResolve: ((snapshot: MockSnapshot) => void) | null = null;
const mockFetchFullSnapshot = vi.fn((): Promise<MockSnapshot> => {
  return new Promise((resolve) => {
    fetchResolve = resolve;
  });
});

/**
 * Simulates the resume/snapshot flow from TerminalPane, focusing on the
 * cached-snapshot lifecycle during tab switch.
 *
 * Mirrors the real code in TerminalPane.ts:1022-1042 (resume method).
 */
class ColdSwitchSimulator {
  terminalId: string;
  paused = false;
  cachedSnapshot: MockSnapshot | null = null;
  useDiffSnapshots = true;
  renderedSnapshots: MockSnapshot[] = [];
  fetchStarted = false;
  fetchCompleted = false;

  constructor(terminalId: string) {
    this.terminalId = terminalId;
  }

  pause() {
    if (this.paused) return;
    this.paused = true;
    mockDisconnectOutputStream(this.terminalId);
  }

  /**
   * Resume — mirrors real TerminalPane.resume() (lines 1022-1042).
   *
   * Stale-while-revalidate pattern:
   *   1. Preserve cached snapshot for immediate rendering (no black screen)
   *   2. Render stale snapshot instantly so the user sees content
   *   3. Kick off a full fetch in background to get fresh data
   */
  resume() {
    if (!this.paused) return;
    this.paused = false;

    // ─── Stale-while-revalidate: preserve cached snapshot ───
    const staleSnapshot = this.cachedSnapshot;
    if (staleSnapshot) {
      this.renderSnapshot(staleSnapshot);
    }
    // ────────────────────────────────────────────────────────

    mockTriggerProbe(this.terminalId);
    mockConnectOutputStream(this.terminalId);
    this.fetchAndRenderSnapshot(true); // force full fetch after resume
  }

  async fetchAndRenderSnapshot(forceFullFetch = false) {
    this.fetchStarted = true;
    if (!forceFullFetch && this.cachedSnapshot && this.useDiffSnapshots) {
      // Diff path — fast, but requires cached snapshot
      return;
    }
    // Full snapshot path — slow IPC round-trip
    const snapshot = await mockFetchFullSnapshot();
    this.fetchCompleted = true;
    this.cachedSnapshot = snapshot;
    this.renderSnapshot(snapshot);
  }

  renderSnapshot(snapshot: MockSnapshot) {
    this.renderedSnapshots.push(snapshot);
    mockRenderSnapshot(snapshot);
  }

  /**
   * Returns whether the terminal has renderable content right now
   * (i.e., the screen is NOT black).
   */
  hasRenderableContent(): boolean {
    return this.cachedSnapshot !== null;
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #373: Cold switch black screen on tab switch', () => {
  beforeEach(() => {
    mockDisconnectOutputStream.mockClear();
    mockConnectOutputStream.mockClear();
    mockTriggerProbe.mockClear();
    mockRenderSnapshot.mockClear();
    mockFetchFullSnapshot.mockClear();
    fetchResolve = null;
  });

  describe('stale-while-revalidate: immediate rendering on resume', () => {
    it('should have renderable content immediately after resume (before IPC completes)', () => {
      // Bug #373: When switching to a background terminal, the screen is black
      // for 2-3 seconds because resume() nulls cachedSnapshot before the async
      // fetch completes. The cached snapshot should be preserved for immediate
      // rendering.
      const sim = new ColdSwitchSimulator('term-1');
      const staleSnapshot = createMockSnapshot('stale');
      sim.cachedSnapshot = staleSnapshot;

      sim.pause();
      // While paused, the cached snapshot should still exist
      // (the real code preserves it during pause)

      sim.resume();
      // IMMEDIATELY after resume (before any async work completes),
      // the terminal should have renderable content — NOT a black screen.
      expect(sim.hasRenderableContent()).toBe(true);
    });

    it('should render stale snapshot immediately on resume, then update with fresh data', async () => {
      // Bug #373: The correct pattern is "stale-while-revalidate":
      // 1. Render stale cached snapshot immediately (no black screen)
      // 2. Fetch fresh snapshot in background
      // 3. When fresh snapshot arrives, replace the stale one
      const sim = new ColdSwitchSimulator('term-1');
      const staleSnapshot = createMockSnapshot('stale');
      sim.cachedSnapshot = staleSnapshot;

      sim.pause();
      sim.resume();

      // Step 1: Stale snapshot should have been rendered immediately
      expect(sim.renderedSnapshots.length).toBeGreaterThanOrEqual(1);
      expect(sim.renderedSnapshots[0]).toBe(staleSnapshot);

      // Step 2: Background fetch should have started
      expect(sim.fetchStarted).toBe(true);

      // Step 3: Resolve the background fetch with fresh data
      const freshSnapshot = createMockSnapshot('fresh');
      fetchResolve!(freshSnapshot);
      await vi.waitFor(() => expect(sim.fetchCompleted).toBe(true));

      // Step 4: Fresh snapshot should now be the cached one
      expect(sim.cachedSnapshot).toBe(freshSnapshot);
    });

    it('should never have a null cachedSnapshot between pause and fetch completion', async () => {
      // Bug #373: The critical invariant that's violated: between resume() and
      // fetchFullSnapshot() completing, cachedSnapshot should NOT be null.
      // A null cachedSnapshot means the renderer has nothing to paint → black screen.
      const sim = new ColdSwitchSimulator('term-1');
      sim.cachedSnapshot = createMockSnapshot('before-pause');

      sim.pause();
      sim.resume();

      // At this point, the IPC fetch is in-flight but hasn't completed.
      // The cached snapshot should still be available for rendering.
      expect(sim.cachedSnapshot).not.toBeNull();
    });
  });

  describe('multi-terminal cold switch scenario', () => {
    it('switching between 5 terminals: each should have instant content', () => {
      // Bug #373: Typical Godly workflow has 10-20 terminals. When switching
      // between them, each should show content immediately, not a black screen.
      const terminals = Array.from({ length: 5 }, (_, i) => {
        const sim = new ColdSwitchSimulator(`term-${i}`);
        sim.cachedSnapshot = createMockSnapshot(`content-${i}`);
        return sim;
      });

      // Simulate: all terminals are created, terminal 0 is active
      // Pause terminals 1-4 (they're in background tabs)
      for (let i = 1; i < 5; i++) {
        terminals[i].pause();
      }

      // Switch to terminal 3 (cold switch — it's been paused)
      terminals[0].pause();
      terminals[3].resume();

      // Terminal 3 should have content immediately — no black screen
      expect(terminals[3].hasRenderableContent()).toBe(true);
      // It should have rendered something (the stale snapshot)
      expect(terminals[3].renderedSnapshots.length).toBeGreaterThanOrEqual(1);
    });

    it('rapid tab switching should never show black screen', () => {
      // Bug #373: Users often cycle through tabs quickly. Each switch should
      // show the last-known content instantly, even if it's slightly stale.
      const termA = new ColdSwitchSimulator('term-A');
      const termB = new ColdSwitchSimulator('term-B');
      termA.cachedSnapshot = createMockSnapshot('content-A');
      termB.cachedSnapshot = createMockSnapshot('content-B');

      // Rapid switch: A → B → A → B
      termA.pause();
      termB.resume();
      expect(termB.hasRenderableContent()).toBe(true);

      termB.pause();
      termA.resume();
      expect(termA.hasRenderableContent()).toBe(true);

      termA.pause();
      termB.resume();
      expect(termB.hasRenderableContent()).toBe(true);

      termB.pause();
      termA.resume();
      expect(termA.hasRenderableContent()).toBe(true);
    });
  });

  describe('full fetch still triggered on resume', () => {
    it('should trigger a full snapshot fetch even when stale cache is preserved', () => {
      // The stale snapshot gives instant content, but a full fetch must still
      // happen to get up-to-date terminal state (changes while paused).
      const sim = new ColdSwitchSimulator('term-1');
      sim.cachedSnapshot = createMockSnapshot('stale');

      sim.pause();
      sim.resume();

      // Full fetch should have been triggered
      expect(mockFetchFullSnapshot).toHaveBeenCalled();
    });

    it('full fetch should NOT use diff path after resume (may miss paused changes)', () => {
      // When a terminal is paused, the daemon continues updating its godly-vt
      // parser. A diff snapshot would only contain changes since the last diff
      // was sent, which may miss changes that happened during the pause.
      // The fetch after resume MUST be a full snapshot.
      const sim = new ColdSwitchSimulator('term-1');
      sim.cachedSnapshot = createMockSnapshot('stale');
      sim.useDiffSnapshots = true; // diff is available

      sim.pause();
      sim.resume();

      // The fetch should have taken the full path, not the diff path
      // (because even though we preserved the cached snapshot for rendering,
      // we need a full fetch to get current state)
      expect(mockFetchFullSnapshot).toHaveBeenCalled();
    });
  });
});
