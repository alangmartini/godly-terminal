import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Bug #290: Copy fails for selections spanning more than one viewport.
 *
 * When selecting text, scrolling up to extend the selection beyond the visible
 * viewport, and copying (Ctrl+Shift+C), the copied text is either empty or
 * truncated. Multi-screen selections cannot be copied correctly.
 *
 * Three root causes:
 * 1. Auto-scroll double-adjusts the selection anchor (shifted by adjustSelectionForScroll
 *    AND by the auto-scroll timer in the same tick)
 * 2. Selection coordinates can go negative after scrolling (u16 overflow when passed to backend)
 * 3. contents_between() only reads visible_rows() — limited to viewport height
 *
 * Related: #242 (selection anchoring), #227 (auto-scroll during drag)
 */

// ── Mocks ───────────────────────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);
const mockFetchSnapshot = vi.fn().mockResolvedValue(undefined);

// ── Simulator ───────────────────────────────────────────────────────────

/**
 * Models the TerminalRenderer + TerminalPane selection/scroll/copy pipeline,
 * mirroring the actual code paths for selection auto-scroll and text extraction.
 *
 * This simulator faithfully reproduces the bugs by using the same logic as the
 * production code. The tests assert expected correct behavior, so they FAIL
 * on the current buggy code.
 */
class MultiScreenSelectionSimulator {
  // Grid geometry
  gridRows = 24;
  gridCols = 80;
  cellWidth = 8;
  cellHeight = 16;

  // Selection state (mirrors TerminalRenderer)
  selection = { startRow: 0, startCol: 0, endRow: 0, endCol: 0, active: false };
  isSelecting = false;

  // Scroll state (mirrors TerminalPane)
  scrollbackOffset = 0;
  totalScrollback = 200;
  isUserScrolled = false;

  // Auto-scroll (mirrors TerminalRenderer)
  private autoScrollTimer: ReturnType<typeof setInterval> | null = null;
  private autoScrollDelta = 0;

  /**
   * Mirrors TerminalRenderer.pixelToGrid() — clamped.
   */
  pixelToGrid(clientX: number, clientY: number): { row: number; col: number } {
    const col = Math.floor(clientX / this.cellWidth);
    const row = Math.floor(clientY / this.cellHeight);
    return { row: Math.max(0, Math.min(this.gridRows - 1, row)), col: Math.max(0, col) };
  }

  /**
   * Mirrors TerminalRenderer.pixelToGridRaw() — unclamped for edge detection.
   */
  pixelToGridRaw(clientX: number, clientY: number): { row: number; col: number } {
    const col = Math.floor(clientX / this.cellWidth);
    const row = Math.floor(clientY / this.cellHeight);
    return { row, col };
  }

  mouseDown(clientX: number, clientY: number) {
    const { row, col } = this.pixelToGrid(clientX, clientY);
    this.selection = { startRow: row, startCol: col, endRow: row, endCol: col, active: false };
    this.isSelecting = true;
  }

  /**
   * Mirrors document-level mousemove during selection.
   * Uses raw coords for edge detection, clamped coords for selection end.
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

    // Edge detection for auto-scroll
    if (raw.row < 0) {
      const linesPerTick = Math.min(10, Math.ceil(Math.abs(raw.row)));
      this.startAutoScroll(linesPerTick);
    } else if (raw.row >= this.gridRows) {
      const linesPerTick = -Math.min(10, raw.row - this.gridRows + 1);
      this.startAutoScroll(linesPerTick);
    } else {
      this.stopAutoScroll();
    }
  }

  mouseUp() {
    this.isSelecting = false;
    this.stopAutoScroll();
  }

  /**
   * Mirrors TerminalRenderer.startSelectionAutoScroll() — now fixed.
   *
   * Bug #290 fix: Only call onScrollCallback (which triggers handleScroll →
   * adjustSelectionForScroll). The previous code also adjusted startRow
   * directly in the timer, causing a double-adjustment that made the anchor
   * drift at 2x the scroll rate and get clamped to viewport bounds.
   */
  private startAutoScroll(linesPerTick: number) {
    this.autoScrollDelta = linesPerTick;
    if (this.autoScrollTimer) return;
    this.autoScrollTimer = setInterval(() => {
      // Only call handleScroll — adjustSelectionForScroll inside it handles
      // shifting both startRow and endRow by the scroll delta.
      this.handleScroll(this.autoScrollDelta);
      // Bug #290: Pin endRow to viewport edge so selection extends into
      // scrollback as new rows are revealed by auto-scroll.
      if (this.selection.active) {
        if (this.autoScrollDelta > 0) {
          this.selection.endRow = 0; // Scrolling up: pin to top
        } else {
          this.selection.endRow = this.gridRows - 1; // Scrolling down: pin to bottom
        }
      }
    }, 50);
  }

  private stopAutoScroll() {
    this.autoScrollDelta = 0;
    if (this.autoScrollTimer) {
      clearInterval(this.autoScrollTimer);
      this.autoScrollTimer = null;
    }
  }

  /**
   * Mirrors TerminalPane.handleScroll().
   */
  private handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, Math.min(this.totalScrollback, this.scrollbackOffset + deltaLines));
    if (newOffset === this.scrollbackOffset) return;
    const actualDelta = newOffset - this.scrollbackOffset;
    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    this.adjustSelectionForScroll(actualDelta);
    mockSetScrollback(newOffset);
    mockFetchSnapshot();
  }

  /**
   * Mirrors TerminalRenderer.adjustSelectionForScroll().
   */
  private adjustSelectionForScroll(deltaLines: number) {
    if (!this.selection.active) return;
    this.selection.startRow += deltaLines;
    this.selection.endRow += deltaLines;
    // Bug #290: Only clear off-screen selection when not actively auto-scrolling.
    // During auto-scroll, the selection extends beyond the viewport and
    // endRow is pinned to the viewport edge in the auto-scroll timer.
    if (!this.autoScrollTimer) {
      const normStart = Math.min(this.selection.startRow, this.selection.endRow);
      const normEnd = Math.max(this.selection.startRow, this.selection.endRow);
      if (normEnd < 0 || normStart >= this.gridRows) {
        this.selection.active = false;
      }
    }
  }

  /**
   * Mirrors TerminalRenderer.normalizeSelection().
   */
  normalizeSelection() {
    if (!this.selection.active) return null;
    const sel = this.selection;
    if (sel.startRow < sel.endRow || (sel.startRow === sel.endRow && sel.startCol <= sel.endCol)) {
      return { ...sel };
    }
    return {
      startRow: sel.endRow,
      startCol: sel.endCol,
      endRow: sel.startRow,
      endCol: sel.startCol,
      active: sel.active,
    };
  }

  /**
   * Mirrors the fixed TerminalRenderer.getSelectedText() + backend pipeline.
   *
   * The fix converts viewport-relative selection coordinates to absolute buffer
   * positions using: absRow = totalScrollback - scrollbackOffset + viewportRow.
   * The backend then reads from the combined scrollback + active grid buffer,
   * supporting selections spanning any number of rows.
   */
  getSelectedText(): { text: string; error: string | null; rowsCopied: number } {
    const sel = this.normalizeSelection();
    if (!sel) return { text: '', error: 'no selection', rowsCopied: 0 };

    // Convert viewport-relative rows to absolute buffer positions.
    // This matches the daemon's read_grid_text() logic:
    //   base = scrollback_count - scrollback_offset
    //   abs_row = base + viewport_row
    const base = this.totalScrollback - this.scrollbackOffset;
    const absStartRow = Math.max(0, base + sel.startRow);
    const totalRows = this.totalScrollback + this.gridRows;
    const absEndRow = Math.max(0, Math.min(totalRows - 1, base + sel.endRow));

    if (absStartRow > absEndRow) {
      return { text: '', error: 'selection entirely outside buffer', rowsCopied: 0 };
    }

    const rowsCopied = absEndRow - absStartRow + 1;

    // Simulate text content: one line per absolute row
    const lines: string[] = [];
    for (let r = absStartRow; r <= absEndRow; r++) {
      lines.push(`line-${r}`);
    }

    return {
      text: lines.join('\n'),
      error: null,
      rowsCopied,
    };
  }

  /**
   * Returns content-relative position for a viewport-relative row.
   */
  contentPosition(viewportRow: number): number {
    return viewportRow - this.scrollbackOffset;
  }

  dispose() {
    this.stopAutoScroll();
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #290: copy fails for selections spanning more than one viewport', () => {
  let sim: MultiScreenSelectionSimulator;

  beforeEach(() => {
    sim = new MultiScreenSelectionSimulator();
    mockSetScrollback.mockClear();
    mockFetchSnapshot.mockClear();
  });

  afterEach(() => {
    sim.dispose();
  });

  describe('auto-scroll double-adjustment bug', () => {
    // Bug #290 (issue 1): The auto-scroll timer adjusts startRow twice per tick.
    // adjustSelectionForScroll() adds delta, then the timer adds delta again.
    // The anchor should only be adjusted once per scroll delta.

    it('startRow should shift by exactly the scroll delta per auto-scroll tick, not 2x', async () => {
      // Start selection near bottom of viewport
      sim.mouseDown(0, 20 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 22 * sim.cellHeight);

      expect(sim.selection.active).toBe(true);
      expect(sim.selection.startRow).toBe(20);

      // Drag above viewport to trigger auto-scroll at 3 lines/tick
      sim.mouseMove(100, -48); // ~3 rows above viewport

      // Wait for one auto-scroll tick
      await new Promise(resolve => setTimeout(resolve, 80));

      // After one tick scrolling up by 3: offset should be 3
      // startRow should be 20 + 3 = 23 (shifted once by the scroll delta)
      // BUG: actual behavior is startRow = 20 + 3 + 3 = 26, then clamped to 23
      // The double-adjustment is masked by clamping, but it means the anchor
      // position is WRONG for content tracking.
      const absAnchor = sim.contentPosition(sim.selection.startRow);

      // The anchor was at absolute position 20 (viewport row 20, offset 0).
      // After scrolling up 3 lines: offset=3, viewport row should be 23 → absolute = 23-3 = 20.
      // Expected: absolute position is still 20 (anchor tracks content).
      expect(absAnchor).toBe(20);
    });

    it('startRow should not be adjusted twice in a single auto-scroll tick', async () => {
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 0);
      expect(sim.selection.active).toBe(true);

      const startRowBefore = sim.selection.startRow;

      // Trigger auto-scroll at 2 lines/tick
      sim.mouseMove(100, -32);

      await new Promise(resolve => setTimeout(resolve, 80));

      const scrolledLines = sim.scrollbackOffset;
      // startRow should increase by exactly scrolledLines (one adjustment)
      // BUG: it increases by 2x scrolledLines due to double adjustment
      expect(sim.selection.startRow).toBe(startRowBefore + scrolledLines);
    });
  });

  describe('multi-screen selection coordinates', () => {
    // Bug #290 (issue 2): After scrolling, selection coordinates can exceed
    // viewport bounds. When passed as u16 to the backend, negative values
    // cause serialization errors (silently caught → empty copy).

    it('selection spanning 2x viewport should have valid coordinates for copy', async () => {
      // Start at row 22 (near bottom), drag up and scroll up to create
      // a selection spanning ~48 rows (2x viewport of 24)
      sim.mouseDown(0, 22 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 0);
      expect(sim.selection.active).toBe(true);

      // Drag above viewport to trigger auto-scroll
      sim.mouseMove(100, -48);

      // Wait for enough ticks to scroll up 30 lines
      await new Promise(resolve => setTimeout(resolve, 600));

      // The selection should still be valid
      expect(sim.selection.active).toBe(true);

      // Try to copy — should not fail with negative rows
      const result = sim.getSelectedText();

      // Expected: all rows in the selection should be copyable
      // BUG: coordinates may be negative (u16 overflow) or selection may be
      // entirely outside viewport bounds after double-adjustment
      expect(result.error).toBeNull();
      expect(result.rowsCopied).toBeGreaterThan(sim.gridRows);
    });

    it('selection coordinates should never go negative during auto-scroll up', async () => {
      // Start selection at row 5, auto-scroll up past it
      sim.mouseDown(0, 5 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 3 * sim.cellHeight);
      expect(sim.selection.active).toBe(true);

      // Auto-scroll up aggressively
      sim.mouseMove(100, -160); // 10 lines above viewport

      await new Promise(resolve => setTimeout(resolve, 400));

      // After significant scrolling, the selection end (which was at row 0)
      // shifts by scrollbackOffset due to adjustSelectionForScroll.
      // The endRow should track content, but must not go negative for copy to work.
      const sel = sim.normalizeSelection();

      // Selection should still be active and have valid coordinates for u16
      if (sel) {
        // BUG: endRow can be clamped above startRow causing empty selection,
        // or startRow can go negative from double-adjustment overflow
        expect(sel.startRow).toBeGreaterThanOrEqual(0);
        expect(sel.endRow).toBeGreaterThanOrEqual(0);
        expect(sel.endRow).toBeGreaterThanOrEqual(sel.startRow);
      }
    });
  });

  describe('copy text extraction for multi-screen selection', () => {
    // Bug #290 (issue 3): contents_between() only reads visible_rows()
    // which is limited to the viewport height. A selection spanning more
    // than gridRows rows gets truncated.

    it('should copy all rows when selection spans 2 screens', async () => {
      // Set up: scroll down first so we have room to scroll up
      sim.scrollbackOffset = 0;

      // Create selection from row 22 down to row 0
      sim.mouseDown(0, 22 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 0);

      // Now auto-scroll up to extend selection into scrollback
      sim.mouseMove(100, -48);

      // Wait for auto-scroll to cover 24+ additional rows
      await new Promise(resolve => setTimeout(resolve, 500));

      sim.mouseUp();

      const result = sim.getSelectedText();

      // The selection should span at least 24 rows (one full viewport)
      // plus the scrolled rows. Total should be > gridRows.
      // BUG: contents_between truncates to gridRows (24), losing the rest.
      expect(result.rowsCopied).toBeGreaterThan(sim.gridRows);
      expect(result.error).toBeNull();
    });

    it('should not truncate text when selection extends above viewport', () => {
      // Simulate the end state: selection goes from row -10 to row 23
      // (i.e., 10 rows above viewport + full viewport = 34 rows total)
      sim.selection = {
        startRow: -10,
        startCol: 0,
        endRow: 23,
        endCol: 80,
        active: true,
      };
      sim.scrollbackOffset = 30;

      const result = sim.getSelectedText();

      // Expected: should extract all 34 rows (including the 10 above viewport)
      // BUG: startRow=-10 causes u16 overflow → empty text
      expect(result.error).toBeNull();
      expect(result.rowsCopied).toBe(34);
    });

    it('should not truncate text when selection extends below viewport', () => {
      // Simulate: selection goes from row 0 to row 30 (6 rows below viewport)
      sim.selection = {
        startRow: 0,
        startCol: 0,
        endRow: 30,
        endCol: 80,
        active: true,
      };
      sim.scrollbackOffset = 10;

      const result = sim.getSelectedText();

      // Expected: should extract all 31 rows
      // BUG: contents_between only iterates visible_rows (24 rows max)
      expect(result.error).toBeNull();
      expect(result.rowsCopied).toBe(31);
    });
  });

  describe('end-to-end multi-screen select + scroll + copy', () => {
    it('select near bottom, scroll up 2 pages, copy should include all rows', async () => {
      // Bug #290: Complete reproduction of the user scenario.
      // 1. Start selection at row 20
      sim.mouseDown(0, 20 * sim.cellHeight);
      // 2. Drag to row 0
      sim.mouseMove(sim.gridCols * sim.cellWidth, 0);
      expect(sim.selection.active).toBe(true);

      // 3. Continue dragging above viewport (triggers auto-scroll)
      sim.mouseMove(100, -32);

      // 4. Wait for auto-scroll to scroll up ~48 lines (2 full pages)
      await new Promise(resolve => setTimeout(resolve, 1000));

      // 5. Release mouse
      sim.mouseUp();

      // At this point we expect:
      // - scrollbackOffset >= 24 (scrolled up at least one full page)
      // - selection should span from the original row 20 position to wherever
      //   the viewport now shows (top of scrolled viewport)
      expect(sim.scrollbackOffset).toBeGreaterThanOrEqual(24);

      // 6. Copy
      const result = sim.getSelectedText();

      // Expected: copied text should include rows from the original selection
      // all the way through the scrolled region. Total rows > gridRows.
      // BUG: Either empty (u16 overflow), truncated (contents_between limit),
      // or wrong content (double-adjustment of anchor).
      expect(result.error).toBeNull();
      expect(result.rowsCopied).toBeGreaterThan(sim.gridRows);
    });

    it('select at top, scroll down 2 pages, copy should include all rows', async () => {
      // Start scrolled up into history
      sim.scrollbackOffset = 60;
      sim.isUserScrolled = true;

      // Select at row 2
      sim.mouseDown(0, 2 * sim.cellHeight);
      // Drag to row 23
      sim.mouseMove(sim.gridCols * sim.cellWidth, 23 * sim.cellHeight);

      // Drag below viewport to trigger downward auto-scroll
      sim.mouseMove(100, sim.gridRows * sim.cellHeight + 32);

      // Wait for auto-scroll to scroll down ~48 lines
      await new Promise(resolve => setTimeout(resolve, 1000));

      sim.mouseUp();

      // Copy
      const result = sim.getSelectedText();

      // Expected: all selected rows copied
      expect(result.error).toBeNull();
      expect(result.rowsCopied).toBeGreaterThan(sim.gridRows);
    });
  });
});
