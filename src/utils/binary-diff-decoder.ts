/**
 * Decodes binary-encoded RichGridDiff from the stream:// protocol.
 *
 * Wire format matches the Rust encoder in protocol/src/binary_diff.rs.
 * Uses DataView + TextDecoder for zero-copy parsing.
 */

import type { RichGridDiff, RichGridRow, RichGridCell, CursorState, GridDimensions } from '../components/TerminalRenderer';

// Magic bytes and version
const MAGIC_G = 0x47; // 'G'
const MAGIC_D = 0x44; // 'D'
const EXPECTED_VERSION = 1;

// Flag bits in header
const FLAG_ALTERNATE_SCREEN = 1 << 0;
const FLAG_CURSOR_HIDDEN = 1 << 1;
const FLAG_FULL_REPAINT = 1 << 2;

// Color type tags
const COLOR_DEFAULT = 0x00;
const COLOR_RGB = 0x01;

// Cell attribute bits
const ATTR_BOLD = 1 << 0;
const ATTR_DIM = 1 << 1;
const ATTR_ITALIC = 1 << 2;
const ATTR_UNDERLINE = 1 << 3;
const ATTR_INVERSE = 1 << 4;
const ATTR_WIDE = 1 << 5;
const ATTR_WIDE_CONT = 1 << 6;

// Row flags
const ROW_WRAPPED = 1 << 0;

const textDecoder = new TextDecoder('utf-8');

/**
 * Decode a single binary-encoded RichGridDiff from a Uint8Array.
 * Returns the decoded diff and the number of bytes consumed.
 */
export function decodeBinaryDiff(
  buffer: Uint8Array,
  offset: number = 0,
): { diff: RichGridDiff; bytesRead: number } {
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let pos = offset;

  // Magic
  if (pos + 2 > buffer.length) throw new Error('Unexpected EOF reading magic');
  if (buffer[pos] !== MAGIC_G || buffer[pos + 1] !== MAGIC_D) {
    throw new Error(`Bad magic: 0x${buffer[pos].toString(16)}${buffer[pos + 1].toString(16)}`);
  }
  pos += 2;

  // Version
  if (pos >= buffer.length) throw new Error('Unexpected EOF reading version');
  const version = buffer[pos++];
  if (version !== EXPECTED_VERSION) {
    throw new Error(`Unsupported version: ${version}`);
  }

  // Header fields (all little-endian)
  if (pos + 17 > buffer.length) throw new Error('Unexpected EOF reading header');
  const cursorRow = view.getUint16(pos, true); pos += 2;
  const cursorCol = view.getUint16(pos, true); pos += 2;
  const gridRows = view.getUint16(pos, true); pos += 2;
  const gridCols = view.getUint16(pos, true); pos += 2;
  const flags = buffer[pos++];
  const dirtyRowCount = view.getUint16(pos, true); pos += 2;
  const scrollbackOffset = view.getUint32(pos, true); pos += 4;
  const totalScrollback = view.getUint32(pos, true); pos += 4;

  // Title
  if (pos + 2 > buffer.length) throw new Error('Unexpected EOF reading title length');
  const titleLen = view.getUint16(pos, true); pos += 2;
  if (pos + titleLen > buffer.length) throw new Error('Unexpected EOF reading title');
  const title = titleLen > 0
    ? textDecoder.decode(buffer.subarray(pos, pos + titleLen))
    : '';
  pos += titleLen;

  const cursor: CursorState = { row: cursorRow, col: cursorCol };
  const dimensions: GridDimensions = { rows: gridRows, cols: gridCols };
  const alternateScreen = (flags & FLAG_ALTERNATE_SCREEN) !== 0;
  const cursorHidden = (flags & FLAG_CURSOR_HIDDEN) !== 0;
  const fullRepaint = (flags & FLAG_FULL_REPAINT) !== 0;

  // Dirty rows
  const dirtyRows: [number, RichGridRow][] = new Array(dirtyRowCount);
  for (let r = 0; r < dirtyRowCount; r++) {
    if (pos + 5 > buffer.length) throw new Error('Unexpected EOF reading row header');
    const rowIndex = view.getUint16(pos, true); pos += 2;
    const rowFlags = buffer[pos++];
    const cellCount = view.getUint16(pos, true); pos += 2;
    const wrapped = (rowFlags & ROW_WRAPPED) !== 0;

    const cells: RichGridCell[] = new Array(cellCount);
    for (let c = 0; c < cellCount; c++) {
      // Content
      if (pos >= buffer.length) throw new Error('Unexpected EOF reading cell content length');
      const contentLen = buffer[pos++];
      if (pos + contentLen > buffer.length) throw new Error('Unexpected EOF reading cell content');
      const content = contentLen > 0
        ? textDecoder.decode(buffer.subarray(pos, pos + contentLen))
        : '';
      pos += contentLen;

      // Colors
      const [fg, fgBytes] = decodeColor(buffer, view, pos);
      pos += fgBytes;
      const [bg, bgBytes] = decodeColor(buffer, view, pos);
      pos += bgBytes;

      // Attributes
      if (pos >= buffer.length) throw new Error('Unexpected EOF reading cell attrs');
      const attrs = buffer[pos++];

      cells[c] = {
        content,
        fg,
        bg,
        bold: (attrs & ATTR_BOLD) !== 0,
        dim: (attrs & ATTR_DIM) !== 0,
        italic: (attrs & ATTR_ITALIC) !== 0,
        underline: (attrs & ATTR_UNDERLINE) !== 0,
        inverse: (attrs & ATTR_INVERSE) !== 0,
        wide: (attrs & ATTR_WIDE) !== 0,
        wide_continuation: (attrs & ATTR_WIDE_CONT) !== 0,
      };
    }

    dirtyRows[r] = [rowIndex, { cells, wrapped }];
  }

  const diff: RichGridDiff = {
    dirty_rows: dirtyRows,
    cursor,
    dimensions,
    alternate_screen: alternateScreen,
    cursor_hidden: cursorHidden,
    title,
    scrollback_offset: scrollbackOffset,
    total_scrollback: totalScrollback,
    full_repaint: fullRepaint,
  };

  return { diff, bytesRead: pos - offset };
}

/**
 * Decode all concatenated binary diffs from a Uint8Array.
 * The stream:// protocol may deliver multiple diffs in a single chunk.
 */
export function decodeAllDiffs(buffer: Uint8Array): RichGridDiff[] {
  const diffs: RichGridDiff[] = [];
  let offset = 0;

  while (offset < buffer.length) {
    // Need at least 2 bytes for magic check
    if (offset + 2 > buffer.length) break;
    // Check for magic — stop if remaining data isn't a valid diff
    if (buffer[offset] !== MAGIC_G || buffer[offset + 1] !== MAGIC_D) break;

    try {
      const { diff, bytesRead } = decodeBinaryDiff(buffer, offset);
      diffs.push(diff);
      offset += bytesRead;
    } catch {
      // Partial/corrupt diff at end of buffer — stop decoding
      break;
    }
  }

  return diffs;
}

// ---- Internal helpers ----

/** Decode a color value, returning [colorString, bytesConsumed]. */
function decodeColor(
  buffer: Uint8Array,
  _view: DataView,
  pos: number,
): [string, number] {
  if (pos >= buffer.length) throw new Error('Unexpected EOF reading color tag');
  const tag = buffer[pos];

  if (tag === COLOR_DEFAULT) {
    return ['default', 1];
  }

  if (tag === COLOR_RGB) {
    if (pos + 4 > buffer.length) throw new Error('Unexpected EOF reading RGB color');
    const r = buffer[pos + 1];
    const g = buffer[pos + 2];
    const b = buffer[pos + 3];
    return [`#${hex(r)}${hex(g)}${hex(b)}`, 4];
  }

  // Unknown tag — treat as default for forward compat
  return ['default', 1];
}

function hex(n: number): string {
  return n.toString(16).padStart(2, '0');
}
