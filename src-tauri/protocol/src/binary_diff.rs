//! Compact binary encoding for `RichGridDiff`.
//!
//! Wire format (~5KB for 80×24 full repaint vs ~50KB JSON):
//!
//! ```text
//! Header:
//!   magic: 2B ("GD")    version: 1B (0x01)
//!   cursor_row: u16LE   cursor_col: u16LE
//!   grid_rows: u16LE    grid_cols: u16LE
//!   flags: u8           (bit0=alternate_screen, bit1=cursor_hidden, bit2=full_repaint)
//!   dirty_row_count: u16LE
//!   scrollback_offset: u32LE   total_scrollback: u32LE
//!   title_len: u16LE + title UTF-8 bytes
//!
//! Per dirty row:
//!   row_index: u16LE    row_flags: u8 (bit0=wrapped)    cell_count: u16LE
//!
//! Per cell:
//!   content_len: u8 + UTF-8 bytes
//!   fg: color encoding (see below)
//!   bg: color encoding (see below)
//!   attrs: u8 (bit0=bold, bit1=dim, bit2=italic, bit3=underline,
//!              bit4=inverse, bit5=wide, bit6=wide_continuation)
//!
//! Color encoding:
//!   0x00 = default
//!   0x01 + 3 bytes = RGB (#rrggbb)
//! ```

use crate::types::{CursorState, GridDimensions, RichGridCell, RichGridDiff, RichGridRow};

const MAGIC: [u8; 2] = [b'G', b'D'];
const VERSION: u8 = 1;

// Flag bits in the header flags byte
const FLAG_ALTERNATE_SCREEN: u8 = 1 << 0;
const FLAG_CURSOR_HIDDEN: u8 = 1 << 1;
const FLAG_FULL_REPAINT: u8 = 1 << 2;

// Color type tags
const COLOR_DEFAULT: u8 = 0x00;
const COLOR_RGB: u8 = 0x01;

// Cell attribute bits
const ATTR_BOLD: u8 = 1 << 0;
const ATTR_DIM: u8 = 1 << 1;
const ATTR_ITALIC: u8 = 1 << 2;
const ATTR_UNDERLINE: u8 = 1 << 3;
const ATTR_INVERSE: u8 = 1 << 4;
const ATTR_WIDE: u8 = 1 << 5;
const ATTR_WIDE_CONT: u8 = 1 << 6;

// Row flag bits
const ROW_WRAPPED: u8 = 1 << 0;

/// Encode a `RichGridDiff` into a compact binary format, appending to `buf`.
pub fn encode_grid_diff_into(diff: &RichGridDiff, buf: &mut Vec<u8>) {
    // Estimate capacity: header ~25B + per row ~5B + per cell ~10B
    let estimated = 25
        + diff.dirty_rows.len() * 5
        + diff
            .dirty_rows
            .iter()
            .map(|(_, r)| r.cells.len() * 10)
            .sum::<usize>();
    buf.reserve(estimated);

    // Header
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION);
    buf.extend_from_slice(&diff.cursor.row.to_le_bytes());
    buf.extend_from_slice(&diff.cursor.col.to_le_bytes());
    buf.extend_from_slice(&diff.dimensions.rows.to_le_bytes());
    buf.extend_from_slice(&diff.dimensions.cols.to_le_bytes());

    let mut flags: u8 = 0;
    if diff.alternate_screen {
        flags |= FLAG_ALTERNATE_SCREEN;
    }
    if diff.cursor_hidden {
        flags |= FLAG_CURSOR_HIDDEN;
    }
    if diff.full_repaint {
        flags |= FLAG_FULL_REPAINT;
    }
    buf.push(flags);

    buf.extend_from_slice(&(diff.dirty_rows.len() as u16).to_le_bytes());
    buf.extend_from_slice(&(diff.scrollback_offset as u32).to_le_bytes());
    buf.extend_from_slice(&(diff.total_scrollback as u32).to_le_bytes());

    // Title
    let title_bytes = diff.title.as_bytes();
    buf.extend_from_slice(&(title_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(title_bytes);

    // Dirty rows
    for (row_idx, row) in &diff.dirty_rows {
        buf.extend_from_slice(&row_idx.to_le_bytes());

        let mut row_flags: u8 = 0;
        if row.wrapped {
            row_flags |= ROW_WRAPPED;
        }
        buf.push(row_flags);

        buf.extend_from_slice(&(row.cells.len() as u16).to_le_bytes());

        for cell in &row.cells {
            encode_cell(cell, buf);
        }
    }
}

/// Encode a `RichGridDiff` into a new `Vec<u8>`.
pub fn encode_grid_diff(diff: &RichGridDiff) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_grid_diff_into(diff, &mut buf);
    buf
}

fn encode_cell(cell: &RichGridCell, buf: &mut Vec<u8>) {
    // Content: length-prefixed UTF-8
    let content_bytes = cell.content.as_bytes();
    buf.push(content_bytes.len() as u8);
    buf.extend_from_slice(content_bytes);

    // Foreground color
    encode_color(&cell.fg, buf);
    // Background color
    encode_color(&cell.bg, buf);

    // Attributes packed into a single byte
    let mut attrs: u8 = 0;
    if cell.bold {
        attrs |= ATTR_BOLD;
    }
    if cell.dim {
        attrs |= ATTR_DIM;
    }
    if cell.italic {
        attrs |= ATTR_ITALIC;
    }
    if cell.underline {
        attrs |= ATTR_UNDERLINE;
    }
    if cell.inverse {
        attrs |= ATTR_INVERSE;
    }
    if cell.wide {
        attrs |= ATTR_WIDE;
    }
    if cell.wide_continuation {
        attrs |= ATTR_WIDE_CONT;
    }
    buf.push(attrs);
}

fn encode_color(color: &str, buf: &mut Vec<u8>) {
    if color == "default" || color.is_empty() {
        buf.push(COLOR_DEFAULT);
    } else if color.len() == 7 && color.starts_with('#') {
        // Parse #rrggbb
        buf.push(COLOR_RGB);
        let r = u8::from_str_radix(&color[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&color[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&color[5..7], 16).unwrap_or(0);
        buf.push(r);
        buf.push(g);
        buf.push(b);
    } else {
        // Unknown format — encode as default
        buf.push(COLOR_DEFAULT);
    }
}

/// Decode error for binary grid diff parsing.
#[derive(Debug)]
pub enum DecodeError {
    /// Not enough bytes to read the expected data.
    UnexpectedEof,
    /// Magic bytes don't match "GD".
    BadMagic,
    /// Unsupported version.
    UnsupportedVersion(u8),
    /// Title bytes are not valid UTF-8.
    InvalidUtf8,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnexpectedEof => write!(f, "unexpected end of data"),
            DecodeError::BadMagic => write!(f, "invalid magic bytes (expected 'GD')"),
            DecodeError::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
            DecodeError::InvalidUtf8 => write!(f, "invalid UTF-8 in string field"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decode a single `RichGridDiff` from a byte slice.
/// Returns the decoded diff and the number of bytes consumed.
pub fn decode_grid_diff(data: &[u8]) -> Result<(RichGridDiff, usize), DecodeError> {
    // Helper closures
    macro_rules! read_u8 {
        ($p:expr) => {{
            if $p >= data.len() {
                return Err(DecodeError::UnexpectedEof);
            }
            let v = data[$p];
            $p += 1;
            v
        }};
    }

    macro_rules! read_u16 {
        ($p:expr) => {{
            if $p + 2 > data.len() {
                return Err(DecodeError::UnexpectedEof);
            }
            let v = u16::from_le_bytes([data[$p], data[$p + 1]]);
            $p += 2;
            v
        }};
    }

    macro_rules! read_u32 {
        ($p:expr) => {{
            if $p + 4 > data.len() {
                return Err(DecodeError::UnexpectedEof);
            }
            let v = u32::from_le_bytes([data[$p], data[$p + 1], data[$p + 2], data[$p + 3]]);
            $p += 4;
            v
        }};
    }

    // Magic
    if data.len() < 2 {
        return Err(DecodeError::UnexpectedEof);
    }
    if data[0] != MAGIC[0] || data[1] != MAGIC[1] {
        return Err(DecodeError::BadMagic);
    }
    let mut pos: usize = 2;

    // Version
    let version = read_u8!(pos);
    if version != VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    // Header fields
    let cursor_row = read_u16!(pos);
    let cursor_col = read_u16!(pos);
    let grid_rows = read_u16!(pos);
    let grid_cols = read_u16!(pos);
    let flags = read_u8!(pos);
    let dirty_row_count = read_u16!(pos);
    let scrollback_offset = read_u32!(pos);
    let total_scrollback = read_u32!(pos);

    // Title
    let title_len = read_u16!(pos) as usize;
    if pos + title_len > data.len() {
        return Err(DecodeError::UnexpectedEof);
    }
    let title = std::str::from_utf8(&data[pos..pos + title_len])
        .map_err(|_| DecodeError::InvalidUtf8)?
        .to_string();
    pos += title_len;

    // Parse flags
    let alternate_screen = flags & FLAG_ALTERNATE_SCREEN != 0;
    let cursor_hidden = flags & FLAG_CURSOR_HIDDEN != 0;
    let full_repaint = flags & FLAG_FULL_REPAINT != 0;

    // Dirty rows
    let mut dirty_rows = Vec::with_capacity(dirty_row_count as usize);
    for _ in 0..dirty_row_count {
        let row_index = read_u16!(pos);
        let row_flags = read_u8!(pos);
        let cell_count = read_u16!(pos) as usize;
        let wrapped = row_flags & ROW_WRAPPED != 0;

        let mut cells = Vec::with_capacity(cell_count);
        for _ in 0..cell_count {
            let content_len = read_u8!(pos) as usize;
            if pos + content_len > data.len() {
                return Err(DecodeError::UnexpectedEof);
            }
            let content = std::str::from_utf8(&data[pos..pos + content_len])
                .map_err(|_| DecodeError::InvalidUtf8)?
                .to_string();
            pos += content_len;

            let fg = decode_color(data, &mut pos)?;
            let bg = decode_color(data, &mut pos)?;

            let attrs = read_u8!(pos);

            cells.push(RichGridCell {
                content,
                fg,
                bg,
                bold: attrs & ATTR_BOLD != 0,
                dim: attrs & ATTR_DIM != 0,
                italic: attrs & ATTR_ITALIC != 0,
                underline: attrs & ATTR_UNDERLINE != 0,
                inverse: attrs & ATTR_INVERSE != 0,
                wide: attrs & ATTR_WIDE != 0,
                wide_continuation: attrs & ATTR_WIDE_CONT != 0,
            });
        }

        dirty_rows.push((row_index, RichGridRow { cells, wrapped }));
    }

    let diff = RichGridDiff {
        dirty_rows,
        cursor: CursorState {
            row: cursor_row,
            col: cursor_col,
            cursor_style: Default::default(),
        },
        dimensions: GridDimensions {
            rows: grid_rows,
            cols: grid_cols,
        },
        alternate_screen,
        cursor_hidden,
        title,
        scrollback_offset: scrollback_offset as usize,
        total_scrollback: total_scrollback as usize,
        full_repaint,
    };

    Ok((diff, pos))
}

fn decode_color(data: &[u8], pos: &mut usize) -> Result<String, DecodeError> {
    if *pos >= data.len() {
        return Err(DecodeError::UnexpectedEof);
    }
    let tag = data[*pos];
    *pos += 1;

    match tag {
        COLOR_DEFAULT => Ok("default".to_string()),
        COLOR_RGB => {
            if *pos + 3 > data.len() {
                return Err(DecodeError::UnexpectedEof);
            }
            let r = data[*pos];
            let g = data[*pos + 1];
            let b = data[*pos + 2];
            *pos += 3;
            Ok(format!("#{:02x}{:02x}{:02x}", r, g, b))
        }
        _ => {
            // Unknown tag — treat as default for forward compatibility
            Ok("default".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cell(content: &str, fg: &str, bg: &str) -> RichGridCell {
        RichGridCell {
            content: content.to_string(),
            fg: fg.to_string(),
            bg: bg.to_string(),
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
            wide: false,
            wide_continuation: false,
        }
    }

    fn make_diff(dirty_rows: Vec<(u16, RichGridRow)>) -> RichGridDiff {
        RichGridDiff {
            dirty_rows,
            cursor: CursorState { row: 5, col: 10, cursor_style: Default::default() },
            dimensions: GridDimensions { rows: 24, cols: 80 },
            alternate_screen: false,
            cursor_hidden: false,
            title: String::new(),
            scrollback_offset: 0,
            total_scrollback: 0,
            full_repaint: false,
        }
    }

    #[test]
    fn roundtrip_empty_diff() {
        let diff = make_diff(vec![]);
        let encoded = encode_grid_diff(&diff);
        let (decoded, bytes_read) = decode_grid_diff(&encoded).unwrap();

        assert_eq!(bytes_read, encoded.len());
        assert_eq!(decoded.dirty_rows.len(), 0);
        assert_eq!(decoded.cursor.row, 5);
        assert_eq!(decoded.cursor.col, 10);
        assert_eq!(decoded.dimensions.rows, 24);
        assert_eq!(decoded.dimensions.cols, 80);
        assert!(!decoded.alternate_screen);
        assert!(!decoded.cursor_hidden);
        assert!(!decoded.full_repaint);
    }

    #[test]
    fn roundtrip_single_row() {
        let row = RichGridRow {
            cells: vec![
                make_cell("A", "#cd3131", "default"),
                make_cell("B", "default", "#1e1e1e"),
            ],
            wrapped: false,
        };
        let diff = make_diff(vec![(3, row)]);
        let encoded = encode_grid_diff(&diff);
        let (decoded, _) = decode_grid_diff(&encoded).unwrap();

        assert_eq!(decoded.dirty_rows.len(), 1);
        let (idx, ref row) = decoded.dirty_rows[0];
        assert_eq!(idx, 3);
        assert_eq!(row.cells.len(), 2);
        assert_eq!(row.cells[0].content, "A");
        assert_eq!(row.cells[0].fg, "#cd3131");
        assert_eq!(row.cells[0].bg, "default");
        assert_eq!(row.cells[1].content, "B");
        assert_eq!(row.cells[1].fg, "default");
        assert_eq!(row.cells[1].bg, "#1e1e1e");
    }

    #[test]
    fn roundtrip_all_flags() {
        let mut diff = make_diff(vec![]);
        diff.alternate_screen = true;
        diff.cursor_hidden = true;
        diff.full_repaint = true;
        diff.title = "claude: coding".to_string();
        diff.scrollback_offset = 42;
        diff.total_scrollback = 1000;

        let encoded = encode_grid_diff(&diff);
        let (decoded, _) = decode_grid_diff(&encoded).unwrap();

        assert!(decoded.alternate_screen);
        assert!(decoded.cursor_hidden);
        assert!(decoded.full_repaint);
        assert_eq!(decoded.title, "claude: coding");
        assert_eq!(decoded.scrollback_offset, 42);
        assert_eq!(decoded.total_scrollback, 1000);
    }

    #[test]
    fn roundtrip_cell_attributes() {
        let cell = RichGridCell {
            content: "X".to_string(),
            fg: "#ffffff".to_string(),
            bg: "#000000".to_string(),
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            inverse: true,
            wide: true,
            wide_continuation: false,
        };
        let row = RichGridRow {
            cells: vec![cell],
            wrapped: true,
        };
        let diff = make_diff(vec![(0, row)]);
        let encoded = encode_grid_diff(&diff);
        let (decoded, _) = decode_grid_diff(&encoded).unwrap();

        let (_, ref row) = decoded.dirty_rows[0];
        assert!(row.wrapped);
        let cell = &row.cells[0];
        assert!(cell.bold);
        assert!(cell.dim);
        assert!(cell.italic);
        assert!(cell.underline);
        assert!(cell.inverse);
        assert!(cell.wide);
        assert!(!cell.wide_continuation);
        assert_eq!(cell.fg, "#ffffff");
        assert_eq!(cell.bg, "#000000");
    }

    #[test]
    fn roundtrip_unicode_content() {
        let row = RichGridRow {
            cells: vec![
                make_cell("🦀", "default", "default"),
                make_cell("日", "#ff0000", "default"),
                make_cell("é", "default", "default"),
            ],
            wrapped: false,
        };
        let diff = make_diff(vec![(0, row)]);
        let encoded = encode_grid_diff(&diff);
        let (decoded, _) = decode_grid_diff(&encoded).unwrap();

        let cells = &decoded.dirty_rows[0].1.cells;
        assert_eq!(cells[0].content, "🦀");
        assert_eq!(cells[1].content, "日");
        assert_eq!(cells[2].content, "é");
    }

    #[test]
    fn roundtrip_full_80x24() {
        // Simulate a full 80×24 repaint
        let mut dirty_rows = Vec::new();
        for i in 0..24u16 {
            let mut cells = Vec::new();
            for j in 0..80 {
                cells.push(make_cell(
                    &((b'A' + (j % 26) as u8) as char).to_string(),
                    if j % 2 == 0 { "#cd3131" } else { "default" },
                    "default",
                ));
            }
            dirty_rows.push((
                i,
                RichGridRow {
                    cells,
                    wrapped: false,
                },
            ));
        }

        let mut diff = make_diff(dirty_rows);
        diff.full_repaint = true;

        let encoded = encode_grid_diff(&diff);
        // Binary should be much smaller than JSON
        let json = serde_json::to_string(&diff).unwrap();
        assert!(
            encoded.len() < json.len() / 2,
            "Binary ({} bytes) should be <50% of JSON ({} bytes)",
            encoded.len(),
            json.len()
        );

        let (decoded, _) = decode_grid_diff(&encoded).unwrap();
        assert_eq!(decoded.dirty_rows.len(), 24);
        assert!(decoded.full_repaint);
        for (i, (idx, row)) in decoded.dirty_rows.iter().enumerate() {
            assert_eq!(*idx, i as u16);
            assert_eq!(row.cells.len(), 80);
        }
    }

    #[test]
    fn concatenated_diffs() {
        let diff1 = make_diff(vec![]);
        let diff2 = {
            let mut d = make_diff(vec![]);
            d.cursor.row = 10;
            d
        };

        let mut buf = Vec::new();
        encode_grid_diff_into(&diff1, &mut buf);
        encode_grid_diff_into(&diff2, &mut buf);

        let (d1, consumed1) = decode_grid_diff(&buf).unwrap();
        assert_eq!(d1.cursor.row, 5);

        let (d2, consumed2) = decode_grid_diff(&buf[consumed1..]).unwrap();
        assert_eq!(d2.cursor.row, 10);
        assert_eq!(consumed1 + consumed2, buf.len());
    }

    #[test]
    fn bad_magic_rejected() {
        let mut encoded = encode_grid_diff(&make_diff(vec![]));
        encoded[0] = b'X';
        assert!(matches!(
            decode_grid_diff(&encoded),
            Err(DecodeError::BadMagic)
        ));
    }

    #[test]
    fn truncated_data_rejected() {
        let encoded = encode_grid_diff(&make_diff(vec![]));
        // Truncate to just the magic bytes
        assert!(matches!(
            decode_grid_diff(&encoded[..5]),
            Err(DecodeError::UnexpectedEof)
        ));
    }

    #[test]
    fn encode_into_appends() {
        let diff = make_diff(vec![]);
        let mut buf = vec![0xFF, 0xFE]; // pre-existing data
        encode_grid_diff_into(&diff, &mut buf);
        assert_eq!(buf[0], 0xFF);
        assert_eq!(buf[1], 0xFE);
        assert_eq!(buf[2], b'G');
        assert_eq!(buf[3], b'D');
    }
}
