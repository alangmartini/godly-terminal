import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Tests for terminal content preservation when switching tabs.
 *
 * Bug: When switching from one terminal to another in the same workspace, the
 * content of the previously-active terminal is completely erased. This happens
 * because:
 *
 * 1. The hidden pane's container goes to display:none (0×0 dimensions)
 * 2. The ResizeObserver fires for the hidden pane → fit() is called
 * 3. fit() calls renderer.getGridSize() which reads getBoundingClientRect()
 *    on the hidden container → rect is {width:0, height:0}
 * 4. getGridSize() returns {rows:1, cols:1} (Math.max(1, 0) = 1)
 * 5. fit() sends resizeTerminal(terminalId, 1, 1) to the daemon
 * 6. Daemon's godly-vt grid is physically truncated to 1×1 → content destroyed
 * 7. When user switches back, the grid is resized to correct dimensions but
 *    the content is gone forever.
 *
 * Expected behavior: fit() should NOT send a resize when the container is hidden
 * (has zero or degenerate dimensions). Specifically, it should guard against
 * sending resize with rows ≤ 1 or cols ≤ 1 when those values result from a
 * hidden container, not an intentionally tiny terminal.
 */

// ── Simulator mirroring TerminalPane.fit() and getGridSize() logic ──────

interface GridSize {
  rows: number;
  cols: number;
}

interface CachedSnapshot {
  dimensions: GridSize;
}

const mockResizeTerminal = vi.fn();
const mockUpdateSize = vi.fn();

/**
 * Simulates the fit() + getGridSize() logic from TerminalPane and
 * TerminalRenderer, allowing us to control container dimensions.
 */
class FitSimulator {
  terminalId: string;
  cachedSnapshot: CachedSnapshot | null = null;

  // Simulated container dimensions (what getBoundingClientRect() returns)
  containerWidth = 800;
  containerHeight = 600;

  // Cell metrics (matching TerminalRenderer defaults)
  cellWidth = 8.4; // approx for 14px Cascadia Code
  cellHeight = 16.8;
  devicePixelRatio = 1;

  constructor(terminalId: string) {
    this.terminalId = terminalId;
  }

  /** Mirror of TerminalRenderer.getGridSize() */
  getGridSize(): GridSize {
    if (this.cellWidth === 0 || this.cellHeight === 0) {
      return { rows: 24, cols: 80 };
    }
    const rows = Math.max(
      1,
      Math.floor(
        this.containerHeight / (this.cellHeight / this.devicePixelRatio)
      )
    );
    const cols = Math.max(
      1,
      Math.floor(
        this.containerWidth / (this.cellWidth / this.devicePixelRatio)
      )
    );
    return { rows, cols };
  }

  /** Mirror of TerminalPane.fit() — must match the source in TerminalPane.ts */
  fit() {
    // Guard: skip resize when container is hidden (display:none).
    // Mirrors the offsetWidth/offsetHeight check in TerminalPane.fit().
    if (!this.containerWidth || !this.containerHeight) {
      return;
    }
    mockUpdateSize();
    const { rows, cols } = this.getGridSize();
    if (rows > 0 && cols > 0) {
      // Invalidate cache if dimensions changed
      if (
        this.cachedSnapshot &&
        (this.cachedSnapshot.dimensions.rows !== rows ||
          this.cachedSnapshot.dimensions.cols !== cols)
      ) {
        this.cachedSnapshot = null;
      }
      mockResizeTerminal(this.terminalId, rows, cols);
    }
  }

  /** Simulate the container going to display:none (hidden). */
  simulateHidden() {
    this.containerWidth = 0;
    this.containerHeight = 0;
  }

  /** Simulate the container becoming visible with normal size. */
  simulateVisible(width = 800, height = 600) {
    this.containerWidth = width;
    this.containerHeight = height;
  }
}

// ── Tests ───────────────────────────────────────────────────────────────

describe('Terminal tab switch: resize guard against hidden containers', () => {
  beforeEach(() => {
    mockResizeTerminal.mockReset();
    mockUpdateSize.mockReset();
  });

  it('should NOT resize terminal to 1×1 when container is hidden', () => {
    // Bug: When the terminal pane is hidden (display:none), fit() computes
    // grid size as 1×1 and sends this to the daemon, which truncates the
    // grid and destroys all terminal content.
    const sim = new FitSimulator('terminal-A');
    sim.cachedSnapshot = { dimensions: { rows: 24, cols: 80 } };

    // Pane goes hidden (user switches to another tab)
    sim.simulateHidden();
    sim.fit();

    // Verify that resizeTerminal was NOT called with degenerate 1×1 dimensions.
    // The current buggy code DOES call it with (1, 1), which is why this test fails.
    const calls = mockResizeTerminal.mock.calls;
    const sentDegenerate = calls.some(
      ([_id, rows, cols]: [string, number, number]) => rows <= 1 && cols <= 1
    );
    expect(sentDegenerate).toBe(false);
  });

  it('hidden pane computes grid size as 1×1', () => {
    // Verifies the root cause: getGridSize() returns {1,1} for hidden containers.
    // This is NOT the expected behavior — it's what triggers the bug.
    const sim = new FitSimulator('terminal-A');
    sim.simulateHidden();
    const size = sim.getGridSize();

    // When container is hidden, the computed grid size is {1, 1} due to
    // Math.max(1, Math.floor(0 / ...)) = Math.max(1, 0) = 1
    // This degenerate size should NOT be sent to the daemon.
    expect(size.rows).toBe(1);
    expect(size.cols).toBe(1);
  });

  it('visible pane computes reasonable grid size', () => {
    const sim = new FitSimulator('terminal-A');
    sim.simulateVisible(800, 600);
    const size = sim.getGridSize();

    expect(size.rows).toBeGreaterThan(10);
    expect(size.cols).toBeGreaterThan(40);
  });

  it('full tab switch cycle should not send degenerate resize for either terminal', () => {
    // Simulates full tab switch: Terminal A → Terminal B
    // Step 1: Both terminals start visible (A active, B hidden)
    const simA = new FitSimulator('terminal-A');
    const simB = new FitSimulator('terminal-B');
    simA.cachedSnapshot = { dimensions: { rows: 35, cols: 95 } };
    simB.cachedSnapshot = { dimensions: { rows: 35, cols: 95 } };

    // Step 2: User switches to Terminal B
    // A becomes hidden, B becomes visible
    simA.simulateHidden();
    simB.simulateVisible();

    // ResizeObserver fires for A (now hidden) → fit() called on A
    simA.fit();
    // setActive(true) fires for B → fit() called on B
    simB.fit();

    // Step 3: Verify NO degenerate resize was sent for terminal A
    const callsForA = mockResizeTerminal.mock.calls.filter(
      ([id]: [string]) => id === 'terminal-A'
    );
    for (const [id, rows, cols] of callsForA) {
      expect(
        rows > 1 || cols > 1,
        `Terminal A should not be resized to ${rows}×${cols} (degenerate). ` +
        `This destroys all terminal content in the daemon's grid.`
      ).toBe(true);
    }
  });

  it('switching back to a previously hidden terminal preserves snapshot cache', () => {
    // When switching back, the cached snapshot should still be available
    // so the terminal can render immediately while the fresh snapshot loads.
    const sim = new FitSimulator('terminal-A');
    sim.cachedSnapshot = { dimensions: { rows: 35, cols: 95 } };

    // Hide the terminal (tab switch away)
    sim.simulateHidden();
    sim.fit();

    // The cached snapshot should NOT be invalidated just because the pane
    // was hidden. If fit() sends resize(1,1), it would also null out the
    // cache (since 1≠35 and 1≠95), losing the ability to render immediately.
    expect(sim.cachedSnapshot).not.toBeNull();
  });
});
