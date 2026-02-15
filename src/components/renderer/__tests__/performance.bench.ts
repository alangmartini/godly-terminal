import { bench, describe } from 'vitest';
import { CellDataEncoder } from '../CellDataEncoder';
import { ColorCache } from '../ColorCache';
import type { RichGridData, RichGridCell } from '../../TerminalRenderer';
import { DEFAULT_THEME } from '../../TerminalRenderer';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** ANSI 256-color palette sample (hex strings). */
const PALETTE = [
  '#000000', '#cd3131', '#0dbc79', '#e5e510', '#2472c8', '#bc3fbc', '#11a8cd', '#e5e5e5',
  '#666666', '#f14c4c', '#23d18b', '#f5f543', '#3b8eea', '#d670d6', '#29b8db', '#ffffff',
];

/** Characters used to populate cells for a realistic mix. */
const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 /-_=+[]{}()!@#$%^&*';

function makeCell(row: number, col: number): RichGridCell {
  const ci = (row * 131 + col * 37) % CHARS.length;
  const fi = (row * 7 + col * 13) % PALETTE.length;
  const bi = (row * 3 + col * 11) % PALETTE.length;
  return {
    content: CHARS[ci],
    fg: fi === 0 ? 'default' : PALETTE[fi],
    bg: bi === 0 ? 'default' : PALETTE[bi],
    bold: (col % 17) === 0,
    dim: (col % 23) === 0,
    italic: (col % 19) === 0,
    underline: (col % 29) === 0,
    inverse: (col % 31) === 0,
    wide: false,
    wide_continuation: false,
  };
}

function makeSnapshot(rows: number, cols: number): RichGridData {
  const gridRows = [];
  for (let r = 0; r < rows; r++) {
    const cells: RichGridCell[] = [];
    for (let c = 0; c < cols; c++) {
      cells.push(makeCell(r, c));
    }
    gridRows.push({ cells, wrapped: r % 5 === 0 });
  }
  return {
    rows: gridRows,
    cursor: { row: 0, col: 0 },
    dimensions: { rows, cols },
    alternate_screen: false,
    cursor_hidden: false,
    title: '',
    scrollback_offset: 0,
    total_scrollback: 0,
  };
}

// Minimal GlyphAtlas stub matching the interface CellDataEncoder expects.
// Simulates pre-populated ASCII entries with a Map lookup.
class StubGlyphAtlas {
  private glyphs = new Map<string, { x: number; y: number; w: number; h: number }>();

  constructor() {
    // Pre-populate ASCII printable range like the real atlas does
    for (let code = 32; code <= 126; code++) {
      const ch = String.fromCharCode(code);
      for (const bold of [false, true]) {
        for (const italic of [false, true]) {
          const key = `${ch}|${bold ? 1 : 0}|${italic ? 1 : 0}`;
          this.glyphs.set(key, {
            x: (code - 32) * 10,
            y: (bold ? 1 : 0) * 20 + (italic ? 1 : 0) * 40,
            w: 8,
            h: 16,
          });
        }
      }
    }
  }

  getGlyph(char: string, bold: boolean, italic: boolean) {
    const key = `${char}|${bold ? 1 : 0}|${italic ? 1 : 0}`;
    return this.glyphs.get(key) ?? { x: 0, y: 0, w: 8, h: 16 };
  }
}

// ---------------------------------------------------------------------------
// Pre-built fixtures (allocated once, reused across iterations)
// ---------------------------------------------------------------------------

const SNAP_30x80 = makeSnapshot(30, 80);
const SNAP_50x120 = makeSnapshot(50, 120);
const SNAP_100x200 = makeSnapshot(100, 200);

const stubAtlas = new StubGlyphAtlas();

// ---------------------------------------------------------------------------
// 1. CellDataEncoder throughput
// ---------------------------------------------------------------------------

describe('CellDataEncoder throughput', () => {
  bench('encode 30x80 grid', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    encoder.encode(SNAP_30x80, DEFAULT_THEME, stubAtlas as any, colorCache, null);
  });

  bench('encode 50x120 grid', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    encoder.encode(SNAP_50x120, DEFAULT_THEME, stubAtlas as any, colorCache, null);
  });

  bench('encode 100x200 grid', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    encoder.encode(SNAP_100x200, DEFAULT_THEME, stubAtlas as any, colorCache, null);
  });

  bench('encode 30x80 with warm encoder (buffer reuse)', () => {
    // Encoder and cache persist across iterations to measure steady-state
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    // Warm up
    encoder.encode(SNAP_30x80, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    // Measured iteration
    encoder.encode(SNAP_30x80, DEFAULT_THEME, stubAtlas as any, colorCache, null);
  });

  bench('encode 50x120 with selection overlay', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const selection = { startRow: 5, startCol: 0, endRow: 40, endCol: 80, active: true };
    encoder.encode(SNAP_50x120, DEFAULT_THEME, stubAtlas as any, colorCache, selection);
  });
});

// ---------------------------------------------------------------------------
// 2. ColorCache throughput
// ---------------------------------------------------------------------------

describe('ColorCache throughput', () => {
  // 256 unique hex colors
  const uniqueColors: string[] = [];
  for (let i = 0; i < 256; i++) {
    const hex = i.toString(16).padStart(2, '0');
    uniqueColors.push(`#${hex}${hex}${hex}`);
  }

  bench('parse 10,000 colors (cold cache)', () => {
    const cache = new ColorCache();
    for (let i = 0; i < 10_000; i++) {
      cache.parse(uniqueColors[i % uniqueColors.length]);
    }
  });

  bench('parse 10,000 colors (warm cache, all hits)', () => {
    const cache = new ColorCache();
    // Pre-warm
    for (const c of uniqueColors) cache.parse(c);
    // Measured: all cache hits
    for (let i = 0; i < 10_000; i++) {
      cache.parse(uniqueColors[i % uniqueColors.length]);
    }
  });

  bench('dim 10,000 packed colors', () => {
    const cache = new ColorCache();
    for (let i = 0; i < 10_000; i++) {
      cache.dim(((i & 0xFF) << 24) | (((i * 7) & 0xFF) << 16) | (((i * 13) & 0xFF) << 8) | 0xFF);
    }
  });

  bench('parse rgb() format 10,000 times', () => {
    const cache = new ColorCache();
    for (let i = 0; i < 10_000; i++) {
      const r = i % 256;
      const g = (i * 3) % 256;
      const b = (i * 7) % 256;
      cache.parse(`rgb(${r}, ${g}, ${b})`);
    }
  });
});

// ---------------------------------------------------------------------------
// 3. GlyphAtlas lookup throughput
// ---------------------------------------------------------------------------

describe('GlyphAtlas lookup throughput', () => {
  // Use the stub atlas which replicates the Map-based lookup of the real one
  const atlas = new StubGlyphAtlas();
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';

  bench('getGlyph 10,000 ASCII lookups (pre-populated)', () => {
    for (let i = 0; i < 10_000; i++) {
      const ch = chars[i % chars.length];
      atlas.getGlyph(ch, i % 5 === 0, i % 7 === 0);
    }
  });

  bench('getGlyph 10,000 regular (no bold/italic)', () => {
    for (let i = 0; i < 10_000; i++) {
      atlas.getGlyph(chars[i % chars.length], false, false);
    }
  });

  bench('getGlyph 10,000 with bold+italic mix', () => {
    for (let i = 0; i < 10_000; i++) {
      atlas.getGlyph(chars[i % chars.length], true, i % 3 === 0);
    }
  });
});

// ---------------------------------------------------------------------------
// 4. Full render pipeline mock (JS-side cost excluding WebGL calls)
// ---------------------------------------------------------------------------

describe('Full render pipeline mock (encode + upload simulation)', () => {
  bench('encode + typed-array copy for 30x80', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const result = encoder.encode(SNAP_30x80, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    // Simulate the texSubImage2D cost: copy the Uint32Array as the driver would
    const copy = new Uint32Array(result.length);
    copy.set(result);
  });

  bench('encode + typed-array copy for 50x120', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const result = encoder.encode(SNAP_50x120, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    const copy = new Uint32Array(result.length);
    copy.set(result);
  });

  bench('encode + typed-array copy for 100x200', () => {
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    const result = encoder.encode(SNAP_100x200, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    const copy = new Uint32Array(result.length);
    copy.set(result);
  });

  bench('steady-state 50x120: warm encoder + warm cache + warm atlas', () => {
    // This measures the realistic per-frame cost with all caches warm
    const encoder = new CellDataEncoder();
    const colorCache = new ColorCache();
    // Warm up
    encoder.encode(SNAP_50x120, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    // Measured iteration
    const result = encoder.encode(SNAP_50x120, DEFAULT_THEME, stubAtlas as any, colorCache, null);
    const copy = new Uint32Array(result.length);
    copy.set(result);
  });
});
