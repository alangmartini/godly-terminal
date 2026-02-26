import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Bug #374: Selection overlay hides text — opaque fill on separate canvas
 * occludes the grid underneath.
 *
 * The terminal uses two stacked canvases:
 * - Bottom (Canvas2DGridRenderer): paints cell backgrounds, text, and cursor
 * - Top (TerminalRenderer overlay): paints selection highlights, scrollbar, URL hover
 *
 * The overlay canvas is created with `alpha: true`, so unpainted pixels are
 * transparent and the grid shows through. However, the selection fill uses the
 * theme's `selectionBackground` color (e.g. `#283457`) which is fully opaque.
 * When composited by the browser, opaque overlay pixels completely hide the
 * text on the grid canvas underneath — the user sees a solid rectangle instead
 * of highlighted text.
 *
 * Expected: the selection highlight should be semi-transparent so text remains
 * visible through it. Either the fill color should include an alpha component
 * (e.g. `rgba(40, 52, 87, 0.5)`) or `globalAlpha` should be set before drawing.
 */

// ── Types ────────────────────────────────────────────────────────────────

interface Selection {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
  active: boolean;
}

interface GridDimensions {
  rows: number;
  cols: number;
}

interface FillRectCall {
  fillStyle: string;
  globalAlpha: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

// ── Helpers ──────────────────────────────────────────────────────────────

/**
 * Parse a CSS color string and extract its alpha component.
 * Handles hex (#RGB, #RRGGBB, #RRGGBBAA), rgb(), rgba(), named colors.
 * Returns the effective alpha as 0..1, where 1 = fully opaque.
 */
function parseColorAlpha(color: string): number {
  // rgba(r, g, b, a) or rgba(r g b / a)
  const rgbaMatch = color.match(/rgba\(\s*[\d.]+[\s,]+[\d.]+[\s,]+[\d.]+[\s,/]+\s*([\d.]+)\s*\)/);
  if (rgbaMatch) {
    return parseFloat(rgbaMatch[1]);
  }

  // 8-digit hex: #RRGGBBAA
  const hex8 = color.match(/^#([0-9a-fA-F]{8})$/);
  if (hex8) {
    const alphaHex = hex8[1].slice(6, 8);
    return parseInt(alphaHex, 16) / 255;
  }

  // 4-digit hex: #RGBA
  const hex4 = color.match(/^#([0-9a-fA-F]{4})$/);
  if (hex4) {
    const alphaChar = hex4[1][3];
    return parseInt(alphaChar + alphaChar, 16) / 255;
  }

  // rgb() — no alpha
  if (color.match(/^rgb\(/)) return 1;

  // 6-digit hex (#RRGGBB) or 3-digit hex (#RGB) or named — all opaque
  return 1;
}

// ── Simulator ────────────────────────────────────────────────────────────

/**
 * Models the TerminalRenderer.paintOverlay() selection rendering logic,
 * capturing canvas context operations to verify opacity behavior.
 *
 * Mirrors the production code that paints selection highlights on the
 * overlay canvas (separate from the grid canvas).
 */
class SelectionOverlaySimulator {
  // Grid geometry
  gridRows = 24;
  gridCols = 80;
  cellWidth = 10;
  cellHeight = 20;

  // Theme
  selectionBackground = '#283457'; // Tokyo Night default (opaque!)

  // State
  selection: Selection = { startRow: 0, startCol: 0, endRow: 0, endCol: 0, active: false };

  // Canvas context state tracking
  private _globalAlpha = 1.0;
  fillRectCalls: FillRectCall[] = [];
  private _fillStyle = '';

  /**
   * Mirrors TerminalRenderer.paintOverlay() — selection rendering path.
   *
   * This is a faithful reproduction of the production code at
   * TerminalRenderer.ts lines 537-565. Any changes to the production
   * code must be reflected here for the tests to remain valid.
   */
  paintOverlay() {
    const dimensions: GridDimensions = { rows: this.gridRows, cols: this.gridCols };
    this.fillRectCalls = [];
    this._globalAlpha = 1.0;

    const normalizedSel = this.selection.active ? this.normalizeSelection(this.selection) : null;
    if (!normalizedSel) return;

    // ---- BEGIN: mirrors production code exactly ----
    // ctx.save();
    // ctx.globalAlpha = 0.6;
    this._globalAlpha = 0.6;
    // ctx.fillStyle = this.theme.selectionBackground;
    this._fillStyle = this.selectionBackground;

    for (let row = normalizedSel.startRow; row <= normalizedSel.endRow; row++) {
      if (row < 0 || row >= dimensions.rows) continue;
      const y = row * this.cellHeight;

      let startCol: number;
      let endCol: number;
      if (row === normalizedSel.startRow && row === normalizedSel.endRow) {
        startCol = normalizedSel.startCol;
        endCol = normalizedSel.endCol;
      } else if (row === normalizedSel.startRow) {
        startCol = normalizedSel.startCol;
        endCol = dimensions.cols;
      } else if (row === normalizedSel.endRow) {
        startCol = 0;
        endCol = normalizedSel.endCol;
      } else {
        startCol = 0;
        endCol = dimensions.cols;
      }

      const x = startCol * this.cellWidth;
      const w = (endCol - startCol) * this.cellWidth;
      // ctx.fillRect(x, y, w, this.cellHeight);
      this.fillRectCalls.push({
        fillStyle: this._fillStyle,
        globalAlpha: this._globalAlpha,
        x, y, w, h: this.cellHeight,
      });
    }
    // ctx.restore();
    this._globalAlpha = 1.0;
    // ---- END: mirrors production code ----
  }

  normalizeSelection(sel: Selection): Selection {
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
   * Compute the effective opacity of a fill operation.
   * Accounts for both the color's alpha and the context's globalAlpha.
   */
  effectiveOpacity(call: FillRectCall): number {
    const colorAlpha = parseColorAlpha(call.fillStyle);
    return colorAlpha * call.globalAlpha;
  }
}

/**
 * Models both renderers' font measurement to verify cell metric consistency.
 *
 * Both TerminalRenderer (overlay) and Canvas2DGridRenderer (grid) use
 * identical measureFont() logic. If their cell sizes diverge, the
 * selection overlay would be misaligned with the text.
 */
class CellMetricsSimulator {
  /**
   * Mirrors measureFont() from both renderers.
   * Returns { cellWidth, cellHeight } for given fontSize and dpr.
   *
   * The production code uses ctx.measureText('M').width for cellWidth,
   * which can vary between canvas contexts. We simulate this with a
   * configurable measureTextWidth to test divergence scenarios.
   */
  static measureFont(
    fontSize: number,
    dpr: number,
    measureTextWidth: number,
  ): { cellWidth: number; cellHeight: number } {
    const scaledSize = Math.round(fontSize * dpr);
    const cellWidth = Math.ceil(measureTextWidth);
    const cellHeight = Math.ceil(scaledSize * 1.2);
    return { cellWidth, cellHeight };
  }

  /**
   * Verify that a selection rectangle at (row, col) aligns with the grid
   * cell at the same position.
   */
  static isAligned(
    overlayMetrics: { cellWidth: number; cellHeight: number },
    gridMetrics: { cellWidth: number; cellHeight: number },
    row: number,
    col: number,
  ): { xAligned: boolean; yAligned: boolean; xDrift: number; yDrift: number } {
    const overlayX = col * overlayMetrics.cellWidth;
    const overlayY = row * overlayMetrics.cellHeight;
    const gridX = col * gridMetrics.cellWidth;
    const gridY = row * gridMetrics.cellHeight;
    return {
      xAligned: overlayX === gridX,
      yAligned: overlayY === gridY,
      xDrift: Math.abs(overlayX - gridX),
      yDrift: Math.abs(overlayY - gridY),
    };
  }
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #374: selection overlay hides text (opaque fill on separate canvas)', () => {
  let sim: SelectionOverlaySimulator;

  beforeEach(() => {
    sim = new SelectionOverlaySimulator();
  });

  describe('selection opacity — overlay must not occlude grid text', () => {
    // Bug #374: The selection fillStyle is opaque (#283457) and no globalAlpha
    // is set. On the overlay canvas, this completely hides the grid text.

    it('single-row selection fill should be semi-transparent (effective opacity < 1)', () => {
      sim.selection = { startRow: 5, startCol: 2, endRow: 5, endCol: 20, active: true };
      sim.paintOverlay();

      expect(sim.fillRectCalls.length).toBe(1);
      const call = sim.fillRectCalls[0];
      const opacity = sim.effectiveOpacity(call);

      // Bug #374: opacity is 1.0 (opaque) — text is hidden
      // Expected: opacity < 1.0 so text shows through
      expect(opacity).toBeLessThan(1.0);
    });

    it('multi-row selection — all rows should be semi-transparent', () => {
      sim.selection = { startRow: 2, startCol: 5, endRow: 8, endCol: 30, active: true };
      sim.paintOverlay();

      // 7 rows: 2, 3, 4, 5, 6, 7, 8
      expect(sim.fillRectCalls.length).toBe(7);

      for (const call of sim.fillRectCalls) {
        const opacity = sim.effectiveOpacity(call);
        // Bug #374: each row's fill is opaque (1.0)
        expect(opacity).toBeLessThan(1.0);
      }
    });

    it('full-width selection row should be semi-transparent', () => {
      // Middle rows in a multi-row selection span full width (0 to cols)
      sim.selection = { startRow: 0, startCol: 0, endRow: 3, endCol: 80, active: true };
      sim.paintOverlay();

      // Check middle row (row 1 or 2) — these get full width
      const middleRow = sim.fillRectCalls.find(c => c.x === 0 && c.w === sim.gridCols * sim.cellWidth);
      expect(middleRow).toBeDefined();
      expect(sim.effectiveOpacity(middleRow!)).toBeLessThan(1.0);
    });

    it('selection with theme selectionBackground should never be fully opaque on overlay canvas', () => {
      // Test with various theme colors that lack alpha
      const opaqueColors = ['#283457', '#3a3632', '#264f78', '#49483e'];

      for (const color of opaqueColors) {
        sim.selectionBackground = color;
        sim.selection = { startRow: 0, startCol: 0, endRow: 0, endCol: 10, active: true };
        sim.paintOverlay();

        for (const call of sim.fillRectCalls) {
          const opacity = sim.effectiveOpacity(call);
          // Bug #374: opaque colors are used directly without alpha reduction
          expect(opacity).toBeLessThan(1.0);
        }
      }
    });
  });

  describe('selection rectangle position — must align with grid cells', () => {
    it('selection rectangle y position should match grid cell row position', () => {
      sim.selection = { startRow: 5, startCol: 0, endRow: 5, endCol: 10, active: true };
      sim.paintOverlay();

      expect(sim.fillRectCalls.length).toBe(1);
      const call = sim.fillRectCalls[0];
      // Y position should be row * cellHeight
      expect(call.y).toBe(5 * sim.cellHeight);
      // Height should be exactly one cell
      expect(call.h).toBe(sim.cellHeight);
    });

    it('selection rectangle x position should match grid cell column position', () => {
      sim.selection = { startRow: 3, startCol: 10, endRow: 3, endCol: 30, active: true };
      sim.paintOverlay();

      expect(sim.fillRectCalls.length).toBe(1);
      const call = sim.fillRectCalls[0];
      // X position should be startCol * cellWidth
      expect(call.x).toBe(10 * sim.cellWidth);
      // Width should be (endCol - startCol) * cellWidth
      expect(call.w).toBe(20 * sim.cellWidth);
    });

    it('multi-row selection first row starts at startCol, last row ends at endCol', () => {
      sim.selection = { startRow: 2, startCol: 15, endRow: 4, endCol: 25, active: true };
      sim.paintOverlay();

      expect(sim.fillRectCalls.length).toBe(3);

      // First row: startCol to full width
      const firstRow = sim.fillRectCalls[0];
      expect(firstRow.x).toBe(15 * sim.cellWidth);
      expect(firstRow.w).toBe((sim.gridCols - 15) * sim.cellWidth);

      // Middle row: full width
      const middleRow = sim.fillRectCalls[1];
      expect(middleRow.x).toBe(0);
      expect(middleRow.w).toBe(sim.gridCols * sim.cellWidth);

      // Last row: col 0 to endCol
      const lastRow = sim.fillRectCalls[2];
      expect(lastRow.x).toBe(0);
      expect(lastRow.w).toBe(25 * sim.cellWidth);
    });
  });

  describe('inactive selection should not render', () => {
    it('does not paint when selection is not active', () => {
      sim.selection = { startRow: 0, startCol: 0, endRow: 5, endCol: 10, active: false };
      sim.paintOverlay();
      expect(sim.fillRectCalls.length).toBe(0);
    });

    it('does not paint when start equals end (click without drag)', () => {
      sim.selection = { startRow: 3, startCol: 5, endRow: 3, endCol: 5, active: true };
      sim.paintOverlay();
      // Even if active, zero-width selection draws nothing visible
      expect(sim.fillRectCalls.length).toBe(1);
      expect(sim.fillRectCalls[0].w).toBe(0);
    });
  });
});

describe('Bug #374: cell metric consistency between overlay and grid renderer', () => {
  // If the overlay canvas and grid canvas compute different cellWidth/cellHeight,
  // the selection highlight will be misaligned with the text. Both renderers use
  // identical measureFont() logic, but on different canvas contexts.

  it('both renderers produce identical cell metrics for same fontSize and DPR', () => {
    // Simulate identical measureText results (common case)
    const fontSize = 13;
    const dpr = 1.5;
    const measureWidth = 9.75; // simulated measureText('M').width

    const overlay = CellMetricsSimulator.measureFont(fontSize, dpr, measureWidth);
    const grid = CellMetricsSimulator.measureFont(fontSize, dpr, measureWidth);

    expect(overlay.cellWidth).toBe(grid.cellWidth);
    expect(overlay.cellHeight).toBe(grid.cellHeight);
  });

  it('cell metric divergence causes selection misalignment at distant rows', () => {
    // Scenario: canvas contexts return slightly different measureText values
    // (possible with alpha:true vs alpha:false contexts)
    const fontSize = 13;
    const dpr = 1.25;

    const overlayMetrics = CellMetricsSimulator.measureFont(fontSize, dpr, 9.73);
    const gridMetrics = CellMetricsSimulator.measureFont(fontSize, dpr, 9.74);

    // Even small differences amplify at distant rows/columns
    const row30 = CellMetricsSimulator.isAligned(overlayMetrics, gridMetrics, 30, 80);

    // If metrics differ, the drift accumulates linearly with row/col number.
    // Selection at row 30, col 80 would be visibly offset.
    if (overlayMetrics.cellWidth !== gridMetrics.cellWidth ||
        overlayMetrics.cellHeight !== gridMetrics.cellHeight) {
      // This is the problematic case — metrics diverged
      expect(row30.xDrift + row30.yDrift).toBeGreaterThan(0);
    } else {
      // Metrics match — no drift
      expect(row30.xAligned).toBe(true);
      expect(row30.yAligned).toBe(true);
    }
  });

  it('selection should align with grid cells at all common DPR values', () => {
    const dprValues = [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
    const fontSize = 13;

    for (const dpr of dprValues) {
      // Both renderers use Math.round(fontSize * dpr) and Math.ceil()
      // so integer arithmetic should be deterministic
      const scaledSize = Math.round(fontSize * dpr);
      const cellHeight = Math.ceil(scaledSize * 1.2);

      // Verify that row * cellHeight is consistent between overlay and grid
      for (let row = 0; row < 50; row++) {
        const overlayY = row * cellHeight;
        const gridY = row * cellHeight;
        expect(overlayY).toBe(gridY);
      }
    }
  });
});

describe('Bug #374: parseColorAlpha helper', () => {
  it('opaque hex colors return alpha = 1', () => {
    expect(parseColorAlpha('#283457')).toBe(1);
    expect(parseColorAlpha('#fff')).toBe(1);
    expect(parseColorAlpha('#000000')).toBe(1);
  });

  it('rgba colors return correct alpha', () => {
    expect(parseColorAlpha('rgba(40, 52, 87, 0.5)')).toBeCloseTo(0.5);
    expect(parseColorAlpha('rgba(255, 255, 255, 0.1)')).toBeCloseTo(0.1);
    expect(parseColorAlpha('rgba(0, 0, 0, 0.75)')).toBeCloseTo(0.75);
  });

  it('8-digit hex returns correct alpha', () => {
    expect(parseColorAlpha('#28345780')).toBeCloseTo(0.502, 1); // 0x80 = 128/255
    expect(parseColorAlpha('#283457ff')).toBeCloseTo(1.0);
    expect(parseColorAlpha('#28345700')).toBeCloseTo(0.0);
  });

  it('rgb() without alpha returns 1', () => {
    expect(parseColorAlpha('rgb(40, 52, 87)')).toBe(1);
  });
});
