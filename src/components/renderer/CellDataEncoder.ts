import type { RichGridData, TerminalTheme } from '../TerminalRenderer';
import type { GlyphAtlas, GlyphEntry } from './GlyphAtlas';
import { ColorCache } from './ColorCache';

export interface Selection {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
  active: boolean;
}

/**
 * Cell data texture format (RGBA32UI, one texel per cell = 4 uint32s):
 *   [0] R: fg_color  (0xRRGGBBFF)
 *   [1] G: bg_color  (0xRRGGBBFF)
 *   [2] B: atlas_x | (atlas_y << 16)
 *   [3] A: glyph_width | (glyph_height << 8) | (flags << 16)
 *
 * Flags byte bits:
 *   0: bold, 1: italic, 2: underline, 3: dim,
 *   4: inverse, 5: wide, 6: wide_continuation, 7: selected
 */
export class CellDataEncoder {
  private buffer: Uint32Array = new Uint32Array(0);

  /**
   * Encode a grid snapshot into a Uint32Array for GPU upload.
   * Returns a view into the internal buffer (reused across calls).
   */
  encode(
    snapshot: RichGridData,
    theme: TerminalTheme,
    atlas: GlyphAtlas,
    colorCache: ColorCache,
    selection: Selection | null,
  ): Uint32Array {
    const { rows, cols } = snapshot.dimensions;
    const totalCells = rows * cols;
    const needed = totalCells * 4;

    if (this.buffer.length < needed) {
      this.buffer = new Uint32Array(needed);
    }

    const buf = this.buffer;
    const defaultFg = colorCache.parse(theme.foreground);
    const defaultBg = colorCache.parse(theme.background);

    const sel = selection?.active ? selection : null;

    for (let row = 0; row < rows; row++) {
      const gridRow = snapshot.rows[row];
      for (let col = 0; col < cols; col++) {
        const idx = (row * cols + col) * 4;

        if (!gridRow || col >= gridRow.cells.length) {
          buf[idx] = defaultFg;
          buf[idx + 1] = defaultBg;
          buf[idx + 2] = 0;
          buf[idx + 3] = 0;
          continue;
        }

        const cell = gridRow.cells[col];

        let fg = cell.fg === 'default' ? defaultFg : colorCache.parse(cell.fg);
        let bg = cell.bg === 'default' ? defaultBg : colorCache.parse(cell.bg);

        if (cell.inverse) {
          const tmp = fg;
          fg = bg;
          bg = tmp;
        }

        if (cell.dim) {
          fg = colorCache.dim(fg);
        }

        let flags = 0;
        if (cell.bold) flags |= 1;
        if (cell.italic) flags |= 2;
        if (cell.underline) flags |= 4;
        if (cell.dim) flags |= 8;
        if (cell.inverse) flags |= 16;
        if (cell.wide) flags |= 32;
        if (cell.wide_continuation) flags |= 64;

        if (sel && cellInSelection(row, col, sel)) {
          flags |= 128;
        }

        let atlasX = 0;
        let atlasY = 0;
        let glyphW = 0;
        let glyphH = 0;
        if (cell.content && cell.content !== ' ' && !cell.wide_continuation) {
          const entry: GlyphEntry = atlas.getGlyph(cell.content, cell.bold, cell.italic);
          atlasX = entry.x;
          atlasY = entry.y;
          glyphW = entry.w;
          glyphH = entry.h;
        }

        buf[idx] = fg;
        buf[idx + 1] = bg;
        buf[idx + 2] = (atlasX & 0xFFFF) | ((atlasY & 0xFFFF) << 16);
        buf[idx + 3] = (glyphW & 0xFF) | ((glyphH & 0xFF) << 8) | ((flags & 0xFF) << 16);
      }
    }

    return buf.subarray(0, needed);
  }
}

function cellInSelection(row: number, col: number, sel: Selection): boolean {
  if (row < sel.startRow || row > sel.endRow) return false;
  if (row === sel.startRow && row === sel.endRow) {
    return col >= sel.startCol && col < sel.endCol;
  }
  if (row === sel.startRow) return col >= sel.startCol;
  if (row === sel.endRow) return col < sel.endCol;
  return true;
}
