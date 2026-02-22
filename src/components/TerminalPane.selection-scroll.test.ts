import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Bug #242: Selection moves with viewport when scrolling instead of staying
 * anchored to content.
 *
 * When selecting text and then scrolling (mouse wheel), the selection highlight
 * moves with the viewport instead of staying at the original content position.
 * The selection coordinates are viewport-relative grid indices that are never
 * adjusted when the scrollback offset changes, so the same viewport rows
 * highlight different content after scrolling.
 *
 * Expected: selection stays anchored to its absolute position in the terminal
 * buffer. Viewport-relative coords should shift by the scroll delta so the
 * selection tracks the same content.
 */

// ── Mocks ───────────────────────────────────────────────────────────────

const mockSetScrollback = vi.fn().mockResolvedValue(undefined);
const mockFetchSnapshot = vi.fn().mockResolvedValue(undefined);

// ── Simulator ───────────────────────────────────────────────────────────

/**
 * Models the TerminalRenderer + TerminalPane selection/scroll pipeline,
 * mirroring the actual code paths for selection coordinates and scroll handling.
 *
 * Selection coordinates are viewport-relative (grid row indices), matching
 * TerminalRenderer's Selection interface. The scroll callback mirrors
 * TerminalPane.handleScroll().
 */
class SelectionScrollSimulator {
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

  /**
   * Mirrors TerminalRenderer.pixelToGrid() — clamped.
   */
  pixelToGrid(clientX: number, clientY: number): { row: number; col: number } {
    const col = Math.floor(clientX / this.cellWidth);
    const row = Math.floor(clientY / this.cellHeight);
    return { row: Math.max(0, Math.min(this.gridRows - 1, row)), col: Math.max(0, col) };
  }

  /**
   * Mirrors mousedown handler — starts selection at grid position.
   */
  mouseDown(clientX: number, clientY: number) {
    const { row, col } = this.pixelToGrid(clientX, clientY);
    this.selection = { startRow: row, startCol: col, endRow: row, endCol: col, active: false };
    this.isSelecting = true;
  }

  /**
   * Mirrors mousemove handler during active selection.
   */
  mouseMove(clientX: number, clientY: number) {
    if (!this.isSelecting) return;
    const { row, col } = this.pixelToGrid(clientX, clientY);
    this.selection.endRow = row;
    this.selection.endCol = col;
    if (this.selection.endRow !== this.selection.startRow || this.selection.endCol !== this.selection.startCol) {
      this.selection.active = true;
    }
  }

  /**
   * Mirrors mouseup handler — ends active drag.
   */
  mouseUp() {
    this.isSelecting = false;
  }

  /**
   * Mirrors the wheel scroll path:
   * TerminalRenderer.setupWheelHandler() → onScrollCallback → TerminalPane.handleScroll()
   *
   * deltaLines > 0 = scroll up (into history), < 0 = scroll down (toward live).
   *
   * BUG: This changes scrollbackOffset but does NOT adjust selection coordinates.
   * After scrolling, the same viewport rows show different content, but
   * selection.startRow / selection.endRow still point to the old viewport rows.
   */
  wheelScroll(deltaLines: number) {
    const newOffset = Math.max(0, Math.min(this.totalScrollback, this.scrollbackOffset + deltaLines));
    if (newOffset === this.scrollbackOffset) return;

    // This is the scroll delta that was actually applied (may differ from
    // deltaLines due to clamping at 0 or totalScrollback).
    const actualDelta = newOffset - this.scrollbackOffset;

    this.scrollbackOffset = newOffset;
    this.isUserScrolled = newOffset > 0;
    mockSetScrollback(newOffset);
    mockFetchSnapshot();

    // Bug #242 fix: adjust selection coordinates to stay anchored to content
    this.adjustSelectionForScroll(actualDelta);
  }

  /**
   * Mirrors TerminalRenderer.adjustSelectionForScroll().
   * Shifts selection coordinates so the selection tracks the same absolute
   * content after a viewport scroll. Clears if entirely off-screen.
   */
  private adjustSelectionForScroll(deltaLines: number) {
    if (!this.selection.active) return;
    this.selection.startRow += deltaLines;
    this.selection.endRow += deltaLines;
    // Clear if the entire selection is off-screen
    const normStart = Math.min(this.selection.startRow, this.selection.endRow);
    const normEnd = Math.max(this.selection.startRow, this.selection.endRow);
    if (normEnd < 0 || normStart >= this.gridRows) {
      this.selection.active = false;
    }
  }

  /**
   * Returns a content-relative position for a viewport-relative row.
   * When the selection correctly tracks content across scrolls, this value
   * stays constant: viewportRow - scrollbackOffset.
   *
   * Intuition: scrolling up by N increases both viewportRow and offset by N,
   * so their difference is invariant for the same content.
   */
  contentPosition(viewportRow: number): number {
    return viewportRow - this.scrollbackOffset;
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #242: selection anchor during wheel scroll', () => {
  let sim: SelectionScrollSimulator;

  beforeEach(() => {
    sim = new SelectionScrollSimulator();
    mockSetScrollback.mockClear();
    mockFetchSnapshot.mockClear();
  });

  describe('completed selection + wheel scroll', () => {
    // Bug #242: After completing a selection and scrolling, the selection
    // should stay anchored to the same absolute content position.

    it('selection startRow should track absolute position when scrolling up', () => {
      // Select rows 10-15 in the viewport (absolute rows 10-15 when offset=0)
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      sim.mouseUp();

      expect(sim.selection.active).toBe(true);
      expect(sim.selection.startRow).toBe(10);
      expect(sim.selection.endRow).toBe(15);

      // Record absolute positions before scroll
      const absStartBefore = sim.contentPosition(sim.selection.startRow);
      const absEndBefore = sim.contentPosition(sim.selection.endRow);
      expect(absStartBefore).toBe(10); // offset=0, so absolute=viewport
      expect(absEndBefore).toBe(15);

      // Scroll up 5 lines into history (offset goes from 0 to 5)
      sim.wheelScroll(5);
      expect(sim.scrollbackOffset).toBe(5);

      // After scrolling up 5 lines, the content that was at viewport row 10
      // is now at viewport row 15 (it shifted down by 5). So the selection's
      // viewport-relative coordinates should have increased by 5 to stay on
      // the same content.
      const absStartAfter = sim.contentPosition(sim.selection.startRow);
      const absEndAfter = sim.contentPosition(sim.selection.endRow);

      // EXPECTED: absolute positions are unchanged (selection tracks content)
      expect(absStartAfter).toBe(absStartBefore);
      expect(absEndAfter).toBe(absEndBefore);
    });

    it('selection startRow should track absolute position when scrolling down', () => {
      // Start scrolled up so we can scroll down
      sim.scrollbackOffset = 20;
      sim.isUserScrolled = true;

      // Select rows 10-15 in viewport (absolute rows 30-35)
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      sim.mouseUp();

      const absStartBefore = sim.contentPosition(sim.selection.startRow); // 10-20=-10
      const absEndBefore = sim.contentPosition(sim.selection.endRow);     // 15-20=-5

      // Scroll down 5 lines (offset decreases from 20 to 15)
      sim.wheelScroll(-5);
      expect(sim.scrollbackOffset).toBe(15);

      // After scrolling down 5 lines, content shifted up by 5 in viewport.
      // Selection viewport-relative coords should have decreased by 5.
      const absStartAfter = sim.contentPosition(sim.selection.startRow);
      const absEndAfter = sim.contentPosition(sim.selection.endRow);

      expect(absStartAfter).toBe(absStartBefore);
      expect(absEndAfter).toBe(absEndBefore);
    });

    it('selection should be cleared or clipped when scrolled entirely off-screen', () => {
      // Select rows 0-3 in viewport at offset=0 (absolute rows 0-3)
      sim.mouseDown(0, 0);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 3 * sim.cellHeight);
      sim.mouseUp();

      expect(sim.selection.active).toBe(true);

      // Scroll up 30 lines — the selected content is now far below the viewport
      sim.wheelScroll(30);
      expect(sim.scrollbackOffset).toBe(30);

      // The selection's viewport-relative rows would need to be negative
      // (e.g., startRow = 0 - 30 = -30) to point at the same absolute content.
      // Since viewport rows can't be negative, the selection should either:
      // (a) be cleared (active=false), or
      // (b) have its viewport-relative coords adjusted such that absoluteRow
      //     still matches the original content.
      //
      // Either behavior is acceptable. What's NOT acceptable is the current
      // behavior where the selection stays at viewport rows 0-3 but now
      // highlights completely different content.
      const absStartAfter = sim.contentPosition(sim.selection.startRow);

      // If selection is still active, it must point to the original absolute position
      if (sim.selection.active) {
        expect(absStartAfter).toBe(0); // original absolute row
      }
      // If selection was cleared, that's also acceptable
      // (the test passes either way as long as it doesn't highlight wrong content)
    });

    it('multiple scroll steps should accumulate correctly', () => {
      // Select rows 12-14 at offset=0
      sim.mouseDown(0, 12 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 14 * sim.cellHeight);
      sim.mouseUp();

      const absStartBefore = sim.contentPosition(sim.selection.startRow); // 12
      const absEndBefore = sim.contentPosition(sim.selection.endRow);     // 14

      // Scroll up in 3 small steps
      sim.wheelScroll(3);
      sim.wheelScroll(3);
      sim.wheelScroll(3); // total: offset = 9

      expect(sim.scrollbackOffset).toBe(9);

      const absStartAfter = sim.contentPosition(sim.selection.startRow);
      const absEndAfter = sim.contentPosition(sim.selection.endRow);

      expect(absStartAfter).toBe(absStartBefore);
      expect(absEndAfter).toBe(absEndBefore);
    });
  });

  describe('active selection + wheel scroll', () => {
    // Bug #242: During an active drag selection, wheel-scrolling should keep
    // the selection anchor at its absolute position.

    it('selection anchor should stay fixed when wheel-scrolling during drag', () => {
      // Start selection at row 10
      sim.mouseDown(0, 10 * sim.cellHeight);
      // Drag to row 15 (selection is active, user is still holding mouse)
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);
      expect(sim.selection.active).toBe(true);

      const absAnchorBefore = sim.contentPosition(sim.selection.startRow); // 10

      // User scrolls wheel up 5 lines while still holding mouse button
      sim.wheelScroll(5);
      expect(sim.scrollbackOffset).toBe(5);

      // The anchor (startRow) should still point to the same absolute content
      const absAnchorAfter = sim.contentPosition(sim.selection.startRow);
      expect(absAnchorAfter).toBe(absAnchorBefore);
    });

    it('selection end should follow mouse position after wheel scroll during drag', () => {
      // Start selection at row 10
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 5 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);

      // Scroll up 5 lines
      sim.wheelScroll(5);

      // Move mouse to row 2 (in new viewport)
      sim.mouseMove(sim.gridCols * sim.cellWidth, 2 * sim.cellHeight);

      // endRow should be at viewport row 2 (contentPosition = 2-5 = -3)
      // startRow should still be at content position 10 (viewport row 15, offset 5)
      const absStart = sim.contentPosition(sim.selection.startRow);
      const absEnd = sim.contentPosition(sim.selection.endRow);

      expect(absStart).toBe(10); // anchor stays at original content position
      expect(absEnd).toBe(-3);   // end follows mouse in new viewport (row 2 - offset 5)
    });
  });

  describe('scroll direction and selection coordinate adjustment', () => {
    it('scrolling up should increase viewport-relative selection rows', () => {
      // Select at row 10 (offset=0, absolute=10)
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 12 * sim.cellHeight);
      sim.mouseUp();

      const startRowBefore = sim.selection.startRow; // 10

      // Scroll up 3 lines — content moves down in viewport
      sim.wheelScroll(3);

      // Viewport-relative row should increase by 3 (content shifted down)
      // to keep pointing at the same absolute content
      expect(sim.selection.startRow).toBe(startRowBefore + 3);
    });

    it('scrolling down should decrease viewport-relative selection rows', () => {
      sim.scrollbackOffset = 20;
      sim.isUserScrolled = true;

      // Select at row 10 (offset=20, absolute=30)
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 12 * sim.cellHeight);
      sim.mouseUp();

      const startRowBefore = sim.selection.startRow; // 10

      // Scroll down 3 lines — content moves up in viewport
      sim.wheelScroll(-3);

      // Viewport-relative row should decrease by 3
      expect(sim.selection.startRow).toBe(startRowBefore - 3);
    });
  });
});
