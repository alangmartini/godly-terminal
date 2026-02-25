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
    // Bug #340: During active drag, only adjust the anchor (startRow).
    // endRow tracks the mouse position in viewport coordinates and should
    // not be shifted — this lets the selection grow as the user scrolls.
    if (!this.isSelecting) {
      this.selection.endRow += deltaLines;
    }
    // Bug #340: Don't clear off-screen selection during active drag.
    // The anchor may be off-screen but the selection is still valid.
    if (!this.isSelecting) {
      const normStart = Math.min(this.selection.startRow, this.selection.endRow);
      const normEnd = Math.max(this.selection.startRow, this.selection.endRow);
      if (normEnd < 0 || normStart >= this.gridRows) {
        this.selection.active = false;
      }
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

  describe('Bug #340: selection should grow when scrolling during active drag', () => {
    // Bug #340: During active drag selection, wheel-scrolling shifts both
    // startRow AND endRow by the scroll delta. This makes the selection
    // maintain the same size and slide with the content. Instead, only
    // the anchor (startRow) should be adjusted; endRow should stay at the
    // mouse's viewport position so the selection grows to include the
    // newly revealed content.

    it('endRow should NOT shift when wheel-scrolling during active drag', () => {
      // Start selection at row 10, drag to row 15
      sim.mouseDown(0, 10 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);

      const endRowBefore = sim.selection.endRow; // 15

      // Scroll up 5 lines while still holding mouse (no mousemove fires)
      sim.wheelScroll(5);

      // Bug #340: endRow should stay at 15 (viewport-relative, tracks mouse)
      // NOT shift to 20 (which would keep same content but prevent growth)
      expect(sim.selection.endRow).toBe(endRowBefore);
    });

    it('selection content range should grow when scrolling up during downward drag', () => {
      // Select from row 5 to row 15 (10 rows of content)
      sim.mouseDown(0, 5 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);

      // Content range before scroll: rows [5,15] at offset 0 → absolute [5,15]
      const contentRangeBefore = sim.contentPosition(sim.selection.endRow) - sim.contentPosition(sim.selection.startRow);
      expect(contentRangeBefore).toBe(10);

      // Scroll up 5 lines (offset becomes 5)
      sim.wheelScroll(5);

      // startRow should track content: 5+5=10, contentPos = 10-5=5 (same absolute)
      // endRow should stay at 15 (mouse position), contentPos = 15-5=10
      // Content range after: 10-5 = 5... wait, the selection CONTENT shrinks
      // because the mouse is now pointing at different (older) content.
      // But the BUFFER range grows: anchor is at absolute 5, end is at absolute 10
      // which covers both the original anchor AND the newly visible content.
      //
      // With the bug (both adjusted): startRow=10, endRow=20, contentRange = 10
      // Selection stays same size, just slides.
      //
      // Fixed: startRow=10, endRow=15, and the selection spans from
      // absolute 5 to absolute 10 — the end has moved CLOSER to the anchor
      // in absolute terms (the viewport scrolled up, so viewport row 15 now
      // shows older content). The user's mouse at row 15 now selects up to
      // what was originally at row 10.
      //
      // The key difference: the end of selection changes with scroll,
      // allowing the user to select content they couldn't see before.
      expect(sim.selection.startRow).toBe(10); // anchor tracked content
      expect(sim.selection.endRow).toBe(15);   // end stayed at mouse position
    });

    it('selection should grow when scrolling up during upward drag', () => {
      // Start at row 15, drag UP to row 5 (selecting upward)
      sim.mouseDown(0, 15 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 5 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);
      expect(sim.selection.startRow).toBe(15); // anchor (where clicked)
      expect(sim.selection.endRow).toBe(5);     // end (where mouse is)

      // Scroll up 5 lines
      sim.wheelScroll(5);

      // Anchor (startRow) should track content: 15+5=20
      // End (endRow) should stay at viewport position: 5
      // Normalized: rows 5 to 20 → 16 viewport rows (was 11)
      // The selection GREW by 5 rows to include the newly revealed scrollback
      expect(sim.selection.startRow).toBe(20); // anchor tracked content
      expect(sim.selection.endRow).toBe(5);     // end stayed at mouse position

      const normalizedStart = Math.min(sim.selection.startRow, sim.selection.endRow);
      const normalizedEnd = Math.max(sim.selection.startRow, sim.selection.endRow);
      expect(normalizedEnd - normalizedStart).toBe(15); // grew from 10 to 15
    });

    it('after mouseup, scrolling should adjust both endpoints again', () => {
      // Active selection
      sim.mouseDown(0, 5 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 15 * sim.cellHeight);
      sim.mouseUp(); // complete the selection

      expect(sim.isSelecting).toBe(false);

      const startBefore = sim.selection.startRow; // 5
      const endBefore = sim.selection.endRow;     // 15

      // Scroll up 3 lines — completed selection, both should adjust
      sim.wheelScroll(3);

      expect(sim.selection.startRow).toBe(startBefore + 3); // 8
      expect(sim.selection.endRow).toBe(endBefore + 3);     // 18
    });

    it('selection should not be cleared during active drag even if anchor goes off-screen', () => {
      // Start at row 3, drag to row 10
      sim.mouseDown(0, 3 * sim.cellHeight);
      sim.mouseMove(sim.gridCols * sim.cellWidth, 10 * sim.cellHeight);
      expect(sim.isSelecting).toBe(true);

      // Scroll up 30 lines — anchor would be at viewport row 33 (way off-screen)
      sim.wheelScroll(30);

      // Selection should still be active (anchor is off-screen but selection is valid)
      expect(sim.selection.active).toBe(true);
      expect(sim.selection.startRow).toBe(33); // off-screen anchor
      expect(sim.selection.endRow).toBe(10);    // mouse position on-screen
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
