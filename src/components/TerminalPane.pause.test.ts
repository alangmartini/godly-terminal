import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Tests for pausing/resuming hidden terminal panes (Phase 3, Issue #213).
 *
 * When a terminal is not visible (hidden tab, not in split view), all output
 * processing should be suspended: the output stream is disconnected, snapshot
 * scheduling stops, and event callbacks early-return. The daemon's godly-vt
 * parser keeps state current, so a single snapshot fetch on resume catches up.
 */

// ── Mocks ────────────────────────────────────────────────────────────────

const mockDisconnectOutputStream = vi.fn();
const mockConnectOutputStream = vi.fn();
const mockFetchAndRenderSnapshot = vi.fn();

interface PauseSimulatorSnapshot {
  rows: unknown[];
  cursor: unknown;
  dimensions: { rows: number; cols: number };
}

/**
 * Simulates the pause/resume logic from TerminalPane without needing
 * the real DOM, Tauri IPC, or renderer dependencies.
 */
class PauseSimulator {
  terminalId: string;
  paused = false;
  snapshotPending = false;
  snapshotTimer: ReturnType<typeof setTimeout> | null = null;
  renderRAF: number | null = null;
  cachedSnapshot: PauseSimulatorSnapshot | null = null;
  scheduleSnapshotFetchCalls = 0;

  constructor(terminalId: string) {
    this.terminalId = terminalId;
  }

  pause() {
    if (this.paused) return;
    this.paused = true;
    mockDisconnectOutputStream(this.terminalId);
    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
    }
    this.snapshotPending = false;
    if (this.renderRAF !== null) {
      // In real code: cancelAnimationFrame(this.renderRAF)
      this.renderRAF = null;
    }
  }

  resume() {
    if (!this.paused) return;
    this.paused = false;
    this.cachedSnapshot = null;
    mockConnectOutputStream(this.terminalId);
    mockFetchAndRenderSnapshot(this.terminalId);
  }

  scheduleSnapshotFetch() {
    if (this.paused) return;
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    this.scheduleSnapshotFetchCalls++;
  }

  /** Simulate a grid diff event arriving from the daemon. */
  simulateGridDiffEvent() {
    if (this.paused) return;
    this.scheduleSnapshotFetch();
  }

  /** Simulate a terminal-output event arriving from the daemon. */
  simulateOutputEvent() {
    if (this.paused) return;
    this.scheduleSnapshotFetch();
  }

  /** Simulate an output stream chunk arriving. */
  simulateStreamChunk() {
    if (this.paused) return;
    this.scheduleSnapshotFetch();
  }

  /** Simulate setActive(true) — tab becomes visible. */
  setActive(active: boolean) {
    if (active) {
      this.resume();
    } else {
      this.pause();
    }
  }

  /** Simulate setSplitVisible(visible, focused). */
  setSplitVisible(visible: boolean, _focused: boolean) {
    if (visible) {
      this.resume();
    } else {
      this.pause();
    }
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Terminal pause/resume (Phase 3: pause hidden terminals)', () => {
  beforeEach(() => {
    mockDisconnectOutputStream.mockReset();
    mockConnectOutputStream.mockReset();
    mockFetchAndRenderSnapshot.mockReset();
  });

  describe('pause()', () => {
    it('disconnects the output stream', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      expect(mockDisconnectOutputStream).toHaveBeenCalledWith('term-1');
    });

    it('cancels pending snapshot timer', () => {
      const sim = new PauseSimulator('term-1');
      sim.snapshotTimer = setTimeout(() => {}, 1000);
      sim.snapshotPending = true;

      sim.pause();

      expect(sim.snapshotTimer).toBeNull();
      expect(sim.snapshotPending).toBe(false);
    });

    it('cancels pending render RAF', () => {
      const sim = new PauseSimulator('term-1');
      sim.renderRAF = 42;

      sim.pause();

      expect(sim.renderRAF).toBeNull();
    });

    it('sets paused flag to true', () => {
      const sim = new PauseSimulator('term-1');
      expect(sim.paused).toBe(false);

      sim.pause();

      expect(sim.paused).toBe(true);
    });

    it('is idempotent — calling pause() twice does not disconnect twice', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();
      sim.pause();

      expect(mockDisconnectOutputStream).toHaveBeenCalledTimes(1);
    });
  });

  describe('resume()', () => {
    it('reconnects the output stream', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();
      sim.resume();

      expect(mockConnectOutputStream).toHaveBeenCalledWith('term-1');
    });

    it('fetches a fresh snapshot to catch up', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();
      sim.resume();

      expect(mockFetchAndRenderSnapshot).toHaveBeenCalledWith('term-1');
    });

    it('invalidates cached snapshot to force full fetch (not diff)', () => {
      const sim = new PauseSimulator('term-1');
      sim.cachedSnapshot = {
        rows: [],
        cursor: { row: 0, col: 0 },
        dimensions: { rows: 24, cols: 80 },
      };
      sim.pause();
      sim.resume();

      expect(sim.cachedSnapshot).toBeNull();
    });

    it('sets paused flag to false', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();
      expect(sim.paused).toBe(true);

      sim.resume();

      expect(sim.paused).toBe(false);
    });

    it('is idempotent — calling resume() when not paused is a no-op', () => {
      const sim = new PauseSimulator('term-1');
      sim.resume();

      expect(mockConnectOutputStream).not.toHaveBeenCalled();
      expect(mockFetchAndRenderSnapshot).not.toHaveBeenCalled();
    });
  });

  describe('event callbacks are no-ops when paused', () => {
    it('grid diff events are ignored when paused', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      sim.simulateGridDiffEvent();

      expect(sim.scheduleSnapshotFetchCalls).toBe(0);
    });

    it('terminal-output events are ignored when paused', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      sim.simulateOutputEvent();

      expect(sim.scheduleSnapshotFetchCalls).toBe(0);
    });

    it('output stream chunks are ignored when paused', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      sim.simulateStreamChunk();

      expect(sim.scheduleSnapshotFetchCalls).toBe(0);
    });

    it('scheduleSnapshotFetch() is a no-op when paused', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      sim.scheduleSnapshotFetch();

      expect(sim.snapshotPending).toBe(false);
      expect(sim.scheduleSnapshotFetchCalls).toBe(0);
    });
  });

  describe('tab switch cycle', () => {
    it('setActive(false) pauses, setActive(true) resumes', () => {
      const sim = new PauseSimulator('term-1');

      sim.setActive(false);
      expect(sim.paused).toBe(true);
      expect(mockDisconnectOutputStream).toHaveBeenCalledWith('term-1');

      sim.setActive(true);
      expect(sim.paused).toBe(false);
      expect(mockConnectOutputStream).toHaveBeenCalledWith('term-1');
      expect(mockFetchAndRenderSnapshot).toHaveBeenCalledWith('term-1');
    });

    it('active→inactive→active preserves terminal state availability', () => {
      // Verifies that after a full cycle, the pane is ready to receive output
      const sim = new PauseSimulator('term-1');
      sim.cachedSnapshot = {
        rows: [],
        cursor: { row: 0, col: 0 },
        dimensions: { rows: 24, cols: 80 },
      };

      sim.setActive(false);
      // While paused, events are ignored
      sim.simulateOutputEvent();
      sim.simulateGridDiffEvent();
      expect(sim.scheduleSnapshotFetchCalls).toBe(0);

      sim.setActive(true);
      // After resume, snapshot cache is invalidated for full fetch
      expect(sim.cachedSnapshot).toBeNull();
      // Events work again
      sim.simulateOutputEvent();
      expect(sim.scheduleSnapshotFetchCalls).toBe(1);
    });

    it('no output processing happens between pause and resume', () => {
      const sim = new PauseSimulator('term-1');
      sim.setActive(false);

      // Simulate a burst of output while hidden
      for (let i = 0; i < 100; i++) {
        sim.simulateOutputEvent();
        sim.simulateGridDiffEvent();
        sim.simulateStreamChunk();
      }

      // None of it should have scheduled snapshot fetches
      expect(sim.scheduleSnapshotFetchCalls).toBe(0);
      expect(sim.snapshotPending).toBe(false);
    });
  });

  describe('split view', () => {
    it('both visible panes remain unpaused', () => {
      const simA = new PauseSimulator('term-A');
      const simB = new PauseSimulator('term-B');

      simA.setSplitVisible(true, true);
      simB.setSplitVisible(true, false);

      expect(simA.paused).toBe(false);
      expect(simB.paused).toBe(false);
    });

    it('setSplitVisible(false) pauses the pane', () => {
      const sim = new PauseSimulator('term-1');

      sim.setSplitVisible(false, false);

      expect(sim.paused).toBe(true);
      expect(mockDisconnectOutputStream).toHaveBeenCalledWith('term-1');
    });

    it('setSplitVisible(true) resumes a paused pane', () => {
      const sim = new PauseSimulator('term-1');
      sim.pause();

      sim.setSplitVisible(true, false);

      expect(sim.paused).toBe(false);
      expect(mockConnectOutputStream).toHaveBeenCalledWith('term-1');
      expect(mockFetchAndRenderSnapshot).toHaveBeenCalledWith('term-1');
    });

    it('exiting split mode pauses the non-active pane', () => {
      const simA = new PauseSimulator('term-A');
      const simB = new PauseSimulator('term-B');

      // Both in split view
      simA.setSplitVisible(true, true);
      simB.setSplitVisible(true, false);

      // Exit split: A becomes active tab, B becomes hidden
      simA.setActive(true);
      simB.setSplitVisible(false, false);

      expect(simA.paused).toBe(false);
      expect(simB.paused).toBe(true);
    });
  });

  describe('resume after long pause catches up with full snapshot', () => {
    it('cached snapshot is null after resume, forcing full fetch', () => {
      const sim = new PauseSimulator('term-1');
      sim.cachedSnapshot = {
        rows: [{ cells: [], wrapped: false }],
        cursor: { row: 0, col: 0 },
        dimensions: { rows: 24, cols: 80 },
      };

      sim.pause();
      // Cached snapshot is preserved while paused (for instant re-render if needed)
      expect(sim.cachedSnapshot).not.toBeNull();

      sim.resume();
      // After resume, cache is invalidated so the next fetch is a full snapshot,
      // not a diff (which could miss changes that happened while paused)
      expect(sim.cachedSnapshot).toBeNull();
      expect(mockFetchAndRenderSnapshot).toHaveBeenCalled();
    });
  });
});
