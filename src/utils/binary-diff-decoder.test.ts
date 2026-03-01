import { describe, it, expect } from 'vitest';
import { decodeBinaryDiff, decodeAllDiffs } from './binary-diff-decoder';

/** Build a minimal binary diff buffer by hand (matches Rust encoder format). */
function buildMinimalDiff(opts: {
  cursorRow?: number;
  cursorCol?: number;
  gridRows?: number;
  gridCols?: number;
  flags?: number;
  dirtyRowCount?: number;
  scrollbackOffset?: number;
  totalScrollback?: number;
  title?: string;
  rowData?: Uint8Array;
} = {}): Uint8Array {
  const parts: number[] = [];

  // Magic + version
  parts.push(0x47, 0x44, 0x01);

  // cursor_row (u16LE)
  const cr = opts.cursorRow ?? 5;
  parts.push(cr & 0xff, (cr >> 8) & 0xff);
  // cursor_col (u16LE)
  const cc = opts.cursorCol ?? 10;
  parts.push(cc & 0xff, (cc >> 8) & 0xff);
  // grid_rows (u16LE)
  const gr = opts.gridRows ?? 24;
  parts.push(gr & 0xff, (gr >> 8) & 0xff);
  // grid_cols (u16LE)
  const gc = opts.gridCols ?? 80;
  parts.push(gc & 0xff, (gc >> 8) & 0xff);
  // flags
  parts.push(opts.flags ?? 0);
  // dirty_row_count (u16LE)
  const drc = opts.dirtyRowCount ?? 0;
  parts.push(drc & 0xff, (drc >> 8) & 0xff);
  // scrollback_offset (u32LE)
  const so = opts.scrollbackOffset ?? 0;
  parts.push(so & 0xff, (so >> 8) & 0xff, (so >> 16) & 0xff, (so >> 24) & 0xff);
  // total_scrollback (u32LE)
  const ts = opts.totalScrollback ?? 0;
  parts.push(ts & 0xff, (ts >> 8) & 0xff, (ts >> 16) & 0xff, (ts >> 24) & 0xff);

  // Title
  const titleBytes = new TextEncoder().encode(opts.title ?? '');
  parts.push(titleBytes.length & 0xff, (titleBytes.length >> 8) & 0xff);
  for (const b of titleBytes) parts.push(b);

  // Row data (raw bytes appended as-is)
  if (opts.rowData) {
    for (const b of opts.rowData) parts.push(b);
  }

  return new Uint8Array(parts);
}

/** Build row data for a single row with one cell. */
function buildSingleCellRow(
  rowIndex: number,
  content: string,
  fg: 'default' | [number, number, number],
  bg: 'default' | [number, number, number],
  attrs: number = 0,
  wrapped: boolean = false,
): Uint8Array {
  const parts: number[] = [];

  // row_index (u16LE)
  parts.push(rowIndex & 0xff, (rowIndex >> 8) & 0xff);
  // row_flags
  parts.push(wrapped ? 1 : 0);
  // cell_count (u16LE) = 1
  parts.push(1, 0);

  // Cell: content
  const contentBytes = new TextEncoder().encode(content);
  parts.push(contentBytes.length);
  for (const b of contentBytes) parts.push(b);

  // fg color
  if (fg === 'default') {
    parts.push(0x00);
  } else {
    parts.push(0x01, fg[0], fg[1], fg[2]);
  }

  // bg color
  if (bg === 'default') {
    parts.push(0x00);
  } else {
    parts.push(0x01, bg[0], bg[1], bg[2]);
  }

  // attrs
  parts.push(attrs);

  return new Uint8Array(parts);
}

describe('decodeBinaryDiff', () => {
  it('decodes empty diff', () => {
    const buf = buildMinimalDiff();
    const { diff, bytesRead } = decodeBinaryDiff(buf);

    expect(bytesRead).toBe(buf.length);
    expect(diff.dirty_rows).toHaveLength(0);
    expect(diff.cursor).toEqual({ row: 5, col: 10 });
    expect(diff.dimensions).toEqual({ rows: 24, cols: 80 });
    expect(diff.alternate_screen).toBe(false);
    expect(diff.cursor_hidden).toBe(false);
    expect(diff.full_repaint).toBe(false);
    expect(diff.title).toBe('');
    expect(diff.scrollback_offset).toBe(0);
    expect(diff.total_scrollback).toBe(0);
  });

  it('decodes header flags', () => {
    const buf = buildMinimalDiff({
      flags: 0x07, // all three flags set
      title: 'claude: coding',
      scrollbackOffset: 42,
      totalScrollback: 1000,
    });
    const { diff } = decodeBinaryDiff(buf);

    expect(diff.alternate_screen).toBe(true);
    expect(diff.cursor_hidden).toBe(true);
    expect(diff.full_repaint).toBe(true);
    expect(diff.title).toBe('claude: coding');
    expect(diff.scrollback_offset).toBe(42);
    expect(diff.total_scrollback).toBe(1000);
  });

  it('decodes single cell row', () => {
    const rowData = buildSingleCellRow(3, 'A', [0xcd, 0x31, 0x31], 'default');
    const buf = buildMinimalDiff({ dirtyRowCount: 1, rowData });
    const { diff } = decodeBinaryDiff(buf);

    expect(diff.dirty_rows).toHaveLength(1);
    const [idx, row] = diff.dirty_rows[0];
    expect(idx).toBe(3);
    expect(row.cells).toHaveLength(1);
    expect(row.cells[0].content).toBe('A');
    expect(row.cells[0].fg).toBe('#cd3131');
    expect(row.cells[0].bg).toBe('default');
    expect(row.wrapped).toBe(false);
  });

  it('decodes cell attributes', () => {
    // bold=1, dim=2, italic=4, underline=8, inverse=16, wide=32
    const attrs = 0x3f; // all except wide_continuation
    const rowData = buildSingleCellRow(0, 'X', [0xff, 0xff, 0xff], [0x00, 0x00, 0x00], attrs, true);
    const buf = buildMinimalDiff({ dirtyRowCount: 1, rowData });
    const { diff } = decodeBinaryDiff(buf);

    const cell = diff.dirty_rows[0][1].cells[0];
    expect(cell.bold).toBe(true);
    expect(cell.dim).toBe(true);
    expect(cell.italic).toBe(true);
    expect(cell.underline).toBe(true);
    expect(cell.inverse).toBe(true);
    expect(cell.wide).toBe(true);
    expect(cell.wide_continuation).toBe(false);
    expect(diff.dirty_rows[0][1].wrapped).toBe(true);
  });

  it('decodes unicode content', () => {
    const rowData = buildSingleCellRow(0, '🦀', 'default', 'default');
    const buf = buildMinimalDiff({ dirtyRowCount: 1, rowData });
    const { diff } = decodeBinaryDiff(buf);

    expect(diff.dirty_rows[0][1].cells[0].content).toBe('🦀');
  });

  it('rejects bad magic', () => {
    const buf = buildMinimalDiff();
    buf[0] = 0x58; // 'X'
    expect(() => decodeBinaryDiff(buf)).toThrow('Bad magic');
  });

  it('rejects unsupported version', () => {
    const buf = buildMinimalDiff();
    buf[2] = 99;
    expect(() => decodeBinaryDiff(buf)).toThrow('Unsupported version');
  });

  it('rejects truncated data', () => {
    const buf = buildMinimalDiff();
    expect(() => decodeBinaryDiff(buf.subarray(0, 5))).toThrow('Unexpected EOF');
  });

  it('supports offset parameter', () => {
    const diff = buildMinimalDiff({ cursorRow: 7 });
    const padded = new Uint8Array(10 + diff.length);
    padded.set(diff, 10);

    const { diff: decoded, bytesRead } = decodeBinaryDiff(padded, 10);
    expect(decoded.cursor.row).toBe(7);
    expect(bytesRead).toBe(diff.length);
  });
});

describe('decodeAllDiffs', () => {
  it('decodes empty buffer', () => {
    expect(decodeAllDiffs(new Uint8Array(0))).toHaveLength(0);
  });

  it('decodes single diff', () => {
    const buf = buildMinimalDiff();
    const diffs = decodeAllDiffs(buf);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].cursor.row).toBe(5);
  });

  it('decodes concatenated diffs', () => {
    const d1 = buildMinimalDiff({ cursorRow: 1 });
    const d2 = buildMinimalDiff({ cursorRow: 2 });
    const combined = new Uint8Array(d1.length + d2.length);
    combined.set(d1, 0);
    combined.set(d2, d1.length);

    const diffs = decodeAllDiffs(combined);
    expect(diffs).toHaveLength(2);
    expect(diffs[0].cursor.row).toBe(1);
    expect(diffs[1].cursor.row).toBe(2);
  });

  it('stops at non-magic bytes', () => {
    const d1 = buildMinimalDiff();
    const combined = new Uint8Array(d1.length + 5);
    combined.set(d1, 0);
    combined.set([0xFF, 0xFE, 0xFD, 0xFC, 0xFB], d1.length);

    const diffs = decodeAllDiffs(combined);
    expect(diffs).toHaveLength(1);
  });
});
