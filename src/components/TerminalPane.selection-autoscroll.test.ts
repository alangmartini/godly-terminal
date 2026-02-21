import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Bug #227: No auto-scroll when drag-selecting text beyond viewport edges.
 *
 * When click-dragging to select text and moving the mouse above the top edge
 * (or below the bottom edge) of the terminal viewport, the viewport should
 * auto-scroll in that direction, extending the selection into scrollback.
 *
 * These tests model the interaction between TerminalRenderer mouse handling
 * and TerminalPane scroll management, matching the fixed implementation that
 * uses pixelToGridRaw() for edge detection and document-level listeners.
 */

// ── Mocks ───────────────────────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);
const mockFetchSnapshot = vi.fn().mockResolvedValue(undefined);

// ── Simulator ───────────────────────────────────────────────────────────

/**
 * Models the TerminalRenderer + TerminalPane selection/scroll pipeline.
 *
 * Mirrors the actual code paths:
 * - pixelToGrid: TerminalRenderer.pixelToGrid() (clamped, for rendering)
 * - pixelToGridRaw: TerminalRenderer.pixelToGridRaw() (unclamped, for edge detection)
 * - mousedown/mousemove/mouseup: TerminalRenderer.setupMouseHandlers()
 * - scroll integration: TerminalPane.handleScroll() / handleScrollTo()
 */
class SelectionScrollSimulator {
  // Grid geometry (mirrors TerminalRenderer)
  gridRows = 24;
  gridCols = 80;
  cellWidth = 8;  // pixels per cell (CSS)
  cellHeight = 16; // pixels per cell (CSS)

  // Viewport bounds (canvas rect, in CSS pixels)
  viewportTop = 0;
  viewportLeft = 0;
  get viewportHeight() { return this.gridRows * this.cellHeight; }
  get viewportWidth() { return this.gridCols * this.cellWidth; }

  // Selection state (mirrors TerminalRenderer)
  selection = { startRow: 0, startCol: 0, endRow: 0, endCol: 0, active: false };
  isSelecting = false;

  // Scroll state (mirrors TerminalPane)
  scrollbackOffset = 0;
  totalScrollback = 0;
  isUserScrolled = false;

  // Auto-scroll timer (mirrors TerminalRenderer)
  private autoScrollTimer: ReturnType<typeof setInterval> | null = null;
  private autoScrollDelta = 0;

  // Callbacks
  onScrollCallback: ((deltaLines: number) => void) | null = null;

  constructor() {
    // Wire up the scroll callback to mirror TerminalPane.handleScroll
    this.onScrollCallback = (deltaLines: number) => {
      const newOffset = Math.max(0, Math.min(this.totalScrollback, this.scrollbackOffset + deltaLines));
      if (newOffset === this.scrollbackOffset) return;
      this.scrollbackOffset = newOffset;
      this.isUserScrolled = newOffset > 0;
      mockSetScrollback(newOffset);
      mockFetchSnapshot();
    };
  }

  /**
   * Mirrors TerminalRenderer.pixelToGrid() — clamped for rendering use.
   */
  pixelToGrid(clientX: number, clientY: number): { row: number; col: number } {
    const cssX = clientX - this.viewportLeft;
    const cssY = clientY - this.viewportTop;
    const col = Math.floor(cssX / this.cellWidth);
    const row = Math.floor(cssY / this.cellHeight);
    return { row: Math.max(0, row), col: Math.max(0, col) };
  }

  /**
   * Mirrors TerminalRenderer.pixelToGridRaw() — unclamped for edge detection.
   */
  pixelToGridRaw(clientX: number, clientY: number): { row: number; col: number } {
    const cssX = clientX - this.viewportLeft;
    const cssY = clientY - this.viewportTop;
    const col = Math.floor(cssX / this.cellWidth);
    const row = Math.floor(cssY / this.cellHeight);
    return { row, col };
  }

  /** Mirrors mousedown handler in TerminalRenderer */
  mouseDown(clientX: number, clientY: number) {
    const { row, col } = this.pixelToGrid(clientX, clientY);
    this.selection = { startRow: row, startCol: col, endRow: row, endCol: col, active: false };
    this.isSelecting = true;
  }

  /**
   * Mirrors document-level mousemove handler during selection.
   * Uses pixelToGridRaw() for edge detection and starts/stops auto-scroll.
   */
  mouseMove(clientX: number, clientY: number) {
    if (!this.isSelecting) return;

    const raw = this.pixelToGridRaw(clientX, clientY);
    const clamped = this.pixelToGrid(clientX, clientY);

    this.selection.endRow = Math.min(clamped.row, this.gridRows - 1);
    this.selection.endCol = clamped.col;

    if (this.selection.endRow !== this.selection.startRow || this.selection.endCol !== this.selection.startCol) {
      this.selection.active = true;
    }

    // Edge detection for auto-scroll (mirrors TerminalRenderer)
    if (raw.row < 0) {
      // Mouse above viewport → scroll up into history
      const linesPerTick = Math.min(10, Math.ceil(Math.abs(raw.row)));
      this.startAutoScroll(linesPerTick);
    } else if (raw.row >= this.gridRows) {
      // Mouse below viewport → scroll down toward live
      const linesPerTick = -Math.min(10, raw.row - this.gridRows + 1);
      this.startAutoScroll(linesPerTick);
    } else {
      this.stopAutoScroll();
    }
  }

  /** Mirrors mouseup handler in TerminalRenderer */
  mouseUp() {
    this.isSelecting = false;
    this.stopAutoScroll();
  }

  /** Check if auto-scroll is currently active */
  isAutoScrolling(): boolean {
    return this.autoScrollTimer !== null;
  }

  /** Get the current auto-scroll rate (lines per tick, 0 if not scrolling) */
  getAutoScrollDelta(): number {
    return this.autoScrollDelta;
  }

  private startAutoScroll(linesPerTick: number) {
    this.autoScrollDelta = linesPerTick;
    if (this.autoScrollTimer) return; // already running, just update delta
    this.autoScrollTimer = setInterval(() => {
      if (this.onScrollCallback) {
        this.onScrollCallback(this.autoScrollDelta);
        // Adjust selection anchor (mirrors TerminalRenderer.startSelectionAutoScroll)
        this.selection.startRow = Math.max(0, Math.min(this.gridRows - 1,
          this.selection.startRow + this.autoScrollDelta));
      }
    }, 50); // ~20 fps scroll ticks
  }

  private stopAutoScroll() {
    this.autoScrollDelta = 0;
    if (this.autoScrollTimer) {
      clearInterval(this.autoScrollTimer);
      this.autoScrollTimer = null;
    }
  }

  dispose() {
    this.stopAutoScroll();
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #227: selection auto-scroll on edge drag', () => {
  let sim: SelectionScrollSimulator;

  beforeEach(() => {
    sim = new SelectionScrollSimulator();
    sim.totalScrollback = 200; // plenty of history to scroll into
    mockSetScrollback.mockClear();
    mockFetchSnapshot.mockClear();
  });

  afterEach(() => {
    sim.dispose();
  });

  describe('pixelToGrid edge detection', () => {
    // pixelToGridRaw returns unclamped values for edge detection

    it('should return negative row when mouse is above viewport', () => {
      // Mouse 32px above the top edge → should be row -2
      const { row } = sim.pixelToGridRaw(100, -32);
      expect(row).toBeLessThan(0);
    });

    it('should return row >= gridRows when mouse is below viewport', () => {
      // Mouse 32px below the bottom edge
      const belowY = sim.viewportHeight + 32;
      const { row } = sim.pixelToGridRaw(100, belowY);
      expect(row).toBeGreaterThanOrEqual(sim.gridRows);
    });

    it('should return negative col when mouse is left of viewport', () => {
      const { col } = sim.pixelToGridRaw(-16, 100);
      expect(col).toBeLessThan(0);
    });
  });

  describe('auto-scroll triggers during selection drag', () => {
    it('should trigger scroll-up when dragging above viewport top', () => {
      // Start selection in the middle of the viewport
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Drag above the viewport
      sim.mouseMove(100, -20);

      expect(sim.isAutoScrolling()).toBe(true);
      expect(sim.getAutoScrollDelta()).toBeGreaterThan(0); // positive = scroll up into history
    });

    it('should trigger scroll-down when dragging below viewport bottom', () => {
      // Start scrolled up so there's room to scroll down
      sim.scrollbackOffset = 50;
      sim.isUserScrolled = true;

      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Drag below the viewport
      sim.mouseMove(100, sim.viewportHeight + 20);

      expect(sim.isAutoScrolling()).toBe(true);
      expect(sim.getAutoScrollDelta()).toBeLessThan(0); // negative = scroll down toward live
    });

    it('should NOT trigger auto-scroll when mouse stays within viewport', () => {
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Move within the viewport (still inside bounds)
      sim.mouseMove(100, midY - 50);

      expect(sim.isAutoScrolling()).toBe(false);
    });

    it('should stop auto-scroll when mouse returns inside viewport', () => {
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Drag above viewport → should start auto-scroll
      sim.mouseMove(100, -20);
      expect(sim.isAutoScrolling()).toBe(true);

      // Move back inside viewport → should stop auto-scroll
      sim.mouseMove(100, midY);
      expect(sim.isAutoScrolling()).toBe(false);
    });

    it('should stop auto-scroll on mouseup', () => {
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);
      sim.mouseMove(100, -20);
      expect(sim.isAutoScrolling()).toBe(true);

      sim.mouseUp();
      expect(sim.isAutoScrolling()).toBe(false);
    });
  });

  describe('auto-scroll speed scales with distance', () => {
    it('should scroll faster when mouse is farther above viewport', () => {
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Slightly above → small delta
      sim.mouseMove(100, -10);
      const smallDelta = sim.getAutoScrollDelta();

      // Way above → larger delta
      sim.mouseMove(100, -100);
      const largeDelta = sim.getAutoScrollDelta();

      expect(largeDelta).toBeGreaterThan(smallDelta);
    });

    it('should scroll faster when mouse is farther below viewport', () => {
      sim.scrollbackOffset = 100;
      sim.isUserScrolled = true;

      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);

      // Slightly below
      sim.mouseMove(100, sim.viewportHeight + 10);
      const smallDelta = Math.abs(sim.getAutoScrollDelta());

      // Way below
      sim.mouseMove(100, sim.viewportHeight + 100);
      const largeDelta = Math.abs(sim.getAutoScrollDelta());

      expect(largeDelta).toBeGreaterThan(smallDelta);
    });
  });

  describe('scroll offset updates during auto-scroll', () => {
    it('should increase scrollbackOffset when auto-scrolling up', async () => {
      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);
      sim.mouseMove(100, -20); // drag above viewport

      // Wait for at least one auto-scroll tick
      await new Promise(resolve => setTimeout(resolve, 100));

      expect(mockSetScrollback).toHaveBeenCalled();
      expect(sim.scrollbackOffset).toBeGreaterThan(0);
    });

    it('should decrease scrollbackOffset when auto-scrolling down', async () => {
      sim.scrollbackOffset = 50;
      sim.isUserScrolled = true;

      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);
      sim.mouseMove(100, sim.viewportHeight + 20); // drag below viewport

      await new Promise(resolve => setTimeout(resolve, 100));

      expect(mockSetScrollback).toHaveBeenCalled();
      expect(sim.scrollbackOffset).toBeLessThan(50);
    });

    it('should not scroll past top of scrollback', async () => {
      sim.totalScrollback = 5;

      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);
      sim.mouseMove(100, -200); // far above viewport

      await new Promise(resolve => setTimeout(resolve, 300));

      // Should not exceed totalScrollback
      expect(sim.scrollbackOffset).toBeLessThanOrEqual(sim.totalScrollback);
    });

    it('should not scroll below offset 0', async () => {
      sim.scrollbackOffset = 2;
      sim.isUserScrolled = true;

      const midY = sim.viewportHeight / 2;
      sim.mouseDown(100, midY);
      sim.mouseMove(100, sim.viewportHeight + 200); // far below viewport

      await new Promise(resolve => setTimeout(resolve, 300));

      expect(sim.scrollbackOffset).toBeGreaterThanOrEqual(0);
    });
  });

  describe('selection coordinates extend with scroll', () => {
    it('selection should span more rows than viewport after scrolling up', async () => {
      // Start selection near the bottom of the viewport
      const startRow = sim.gridRows - 2;
      const startY = startRow * sim.cellHeight + sim.cellHeight / 2;
      sim.mouseDown(100, startY);

      // Drag above viewport to trigger auto-scroll
      sim.mouseMove(100, -30);

      // Wait for auto-scroll to move the viewport
      await new Promise(resolve => setTimeout(resolve, 200));

      // The effective selection range (including scrolled rows) should be
      // larger than what fits in the viewport
      const scrolledRows = sim.scrollbackOffset;
      const selectionSpan = startRow + scrolledRows; // rows from start to top of scrolled view

      expect(selectionSpan).toBeGreaterThan(sim.gridRows);
    });
  });
});
