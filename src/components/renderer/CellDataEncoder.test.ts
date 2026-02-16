import { describe, it, expect } from 'vitest';
import { CellDataEncoder } from './CellDataEncoder';
import { ColorCache } from './ColorCache';
import type { RichGridData, TerminalTheme } from '../TerminalRenderer';
import { DEFAULT_THEME } from '../TerminalRenderer';

// Minimal GlyphAtlas stub
const stubAtlas = {
  getGlyph(_char: string, _bold: boolean, _italic: boolean) {
    return { x: 10, y: 20, w: 8, h: 16 };
  },
} as any;

function makeSnapshot(cells: Array<Partial<import('../TerminalRenderer').RichGridCell>>, rows = 1, cols?: number): RichGridData {
  const filledCells = cells.map(c => ({
    content: c.content ?? ' ',
    fg: c.fg ?? 'default',
    bg: c.bg ?? 'default',
    bold: c.bold ?? false,
    dim: c.dim ?? false,
    italic: c.italic ?? false,
    underline: c.underline ?? false,
    inverse: c.inverse ?? false,
    wide: c.wide ?? false,
    wide_continuation: c.wide_continuation ?? false,
  }));
  const actualCols = cols ?? filledCells.length;
  return {
    rows: [{ cells: filledCells, wrapped: false }],
    cursor: { row: 0, col: 0 },
    dimensions: { rows, cols: actualCols },
    alternate_screen: false,
    cursor_hidden: false,
    title: '',
    scrollback_offset: 0,
    total_scrollback: 0,
  };
}

describe('CellDataEncoder', () => {
  it('encodes a single default cell', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: ' ' }]);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    expect(result.length).toBe(4); // 1 cell × 4 uint32s
    // fg = default foreground #cccccc → 0xCCCCCCFF
    expect(result[0]).toBe(0xCCCCCCFF);
    // bg = default background #1e1e1e → 0x1E1E1EFF
    expect(result[1]).toBe(0x1E1E1EFF);
    // Empty space → no atlas entry
    expect(result[2]).toBe(0);
    expect(result[3]).toBe(0);
  });

  it('encodes a cell with text content and atlas lookup', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'A' }]);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    // Atlas coords: x=10, y=20 → (10 & 0xFFFF) | ((20 & 0xFFFF) << 16)
    expect(result[2]).toBe(10 | (20 << 16));
    // Glyph: w=8, h=16, flags=0 → 8 | (16 << 8) | (0 << 16)
    expect(result[3]).toBe(8 | (16 << 8));
  });

  it('encodes bold+italic+underline flags', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'X', bold: true, italic: true, underline: true }]);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    const flags = (result[3] >>> 16) & 0xFF;
    expect(flags & 1).toBe(1);  // bold
    expect(flags & 2).toBe(2);  // italic
    expect(flags & 4).toBe(4);  // underline
  });

  it('encodes inverse by swapping fg/bg', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'I', fg: '#ff0000', bg: '#00ff00', inverse: true }]);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    // fg and bg should be swapped
    expect(result[0]).toBe(0x00FF00FF); // fg = original bg
    expect(result[1]).toBe(0xFF0000FF); // bg = original fg
  });

  it('encodes dim flag and dims the foreground color', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'D', fg: '#ffffff', dim: true }]);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    // Dimmed white: 255 * 0.67 ≈ 171 → 0xABABABFF
    const r = (result[0] >>> 24) & 0xFF;
    expect(r).toBe(171);
    // Dim flag
    const flags = (result[3] >>> 16) & 0xFF;
    expect(flags & 8).toBe(8);
  });

  it('encodes wide and wide_continuation flags', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([
      { content: '中', wide: true },
      { content: '', wide_continuation: true },
    ], 1, 2);
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    const flags0 = (result[3] >>> 16) & 0xFF;
    expect(flags0 & 32).toBe(32); // wide flag on first cell

    const flags1 = (result[7] >>> 16) & 0xFF;
    expect(flags1 & 64).toBe(64); // wide_continuation on second cell
  });

  it('encodes selection flag for cells in selection range', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'A' }, { content: 'B' }, { content: 'C' }], 1, 3);
    const sel = { startRow: 0, startCol: 1, endRow: 0, endCol: 3, active: true };
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, sel);

    // Cell 0 (col 0): not selected
    expect(((result[3] >>> 16) & 0xFF) & 128).toBe(0);
    // Cell 1 (col 1): selected
    expect(((result[7] >>> 16) & 0xFF) & 128).toBe(128);
    // Cell 2 (col 2): selected
    expect(((result[11] >>> 16) & 0xFF) & 128).toBe(128);
  });

  it('handles empty/missing rows gracefully', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    // Snapshot with 2 rows but only 1 actual row
    const snap: RichGridData = {
      rows: [{ cells: [{ content: 'A', fg: 'default', bg: 'default', bold: false, dim: false, italic: false, underline: false, inverse: false, wide: false, wide_continuation: false }], wrapped: false }],
      cursor: { row: 0, col: 0 },
      dimensions: { rows: 2, cols: 1 },
      alternate_screen: false,
      cursor_hidden: false,
      title: '',
      scrollback_offset: 0,
      total_scrollback: 0,
    };
    const result = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);

    expect(result.length).toBe(8); // 2 cells × 4 uint32s
    // Second row should be default colors with no glyph
    expect(result[4]).toBe(0xCCCCCCFF);
    expect(result[5]).toBe(0x1E1E1EFF);
    expect(result[6]).toBe(0);
    expect(result[7]).toBe(0);
  });

  it('reuses buffer across calls', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const snap = makeSnapshot([{ content: 'A' }]);
    const result1 = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);
    const result2 = encoder.encode(snap, DEFAULT_THEME, stubAtlas, colorCache, null);
    // Both should return valid data (buffer is reused internally)
    expect(result1.length).toBe(result2.length);
    expect(result2[0]).toBe(0xCCCCCCFF);
  });
});
