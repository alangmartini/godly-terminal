use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use godly_protocol::types::{
    CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
};

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

/// ANSI 256-color palette sample (hex strings).
const PALETTE: &[&str] = &[
    "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
    "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
];

const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 /-_=+";

fn make_cell(row: usize, col: usize) -> RichGridCell {
    let ci = (row.wrapping_mul(131) + col.wrapping_mul(37)) % CHARS.len();
    let fi = (row.wrapping_mul(7) + col.wrapping_mul(13)) % PALETTE.len();
    let bi = (row.wrapping_mul(3) + col.wrapping_mul(11)) % PALETTE.len();
    RichGridCell {
        content: String::from(CHARS[ci] as char),
        fg: if fi == 0 { "default".into() } else { PALETTE[fi].into() },
        bg: if bi == 0 { "default".into() } else { PALETTE[bi].into() },
        bold: col % 17 == 0,
        dim: col % 23 == 0,
        italic: col % 19 == 0,
        underline: col % 29 == 0,
        inverse: col % 31 == 0,
        wide: false,
        wide_continuation: false,
    }
}

fn make_snapshot(rows: u16, cols: u16) -> RichGridData {
    let grid_rows = (0..rows as usize)
        .map(|r| {
            let cells = (0..cols as usize).map(|c| make_cell(r, c)).collect();
            RichGridRow {
                cells,
                wrapped: r % 5 == 0,
            }
        })
        .collect();

    RichGridData {
        rows: grid_rows,
        cursor: CursorState { row: 0, col: 0 },
        dimensions: GridDimensions { rows, cols },
        alternate_screen: false,
        cursor_hidden: false,
        title: String::new(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

// ---------------------------------------------------------------------------
// Simple binary encoding for comparison
// ---------------------------------------------------------------------------

/// Minimal binary encoding: per-cell layout (fixed 12 bytes):
///   fg_color: [u8; 3], bg_color: [u8; 3], content_byte: u8, flags: u8, padding: [u8; 4]
///
/// This is intentionally naive to show the theoretical floor vs JSON.
fn encode_binary(snapshot: &RichGridData) -> Vec<u8> {
    let total_cells = snapshot.dimensions.rows as usize * snapshot.dimensions.cols as usize;
    // Header: 2 bytes rows + 2 bytes cols + 2 bytes cursor_row + 2 bytes cursor_col = 8 bytes
    let mut buf = Vec::with_capacity(8 + total_cells * 12);

    buf.extend_from_slice(&snapshot.dimensions.rows.to_le_bytes());
    buf.extend_from_slice(&snapshot.dimensions.cols.to_le_bytes());
    buf.extend_from_slice(&snapshot.cursor.row.to_le_bytes());
    buf.extend_from_slice(&snapshot.cursor.col.to_le_bytes());

    for row in &snapshot.rows {
        for cell in &row.cells {
            // Parse fg color (simple hex parse or 0 for "default")
            let (fr, fg, fb) = parse_hex_color(&cell.fg);
            let (br, bg, bb) = parse_hex_color(&cell.bg);
            buf.push(fr);
            buf.push(fg);
            buf.push(fb);
            buf.push(br);
            buf.push(bg);
            buf.push(bb);
            // Content: first byte of UTF-8 (lossy for non-ASCII, fine for benchmark)
            buf.push(cell.content.as_bytes().first().copied().unwrap_or(b' '));
            // Flags packed into one byte
            let mut flags: u8 = 0;
            if cell.bold { flags |= 1; }
            if cell.dim { flags |= 2; }
            if cell.italic { flags |= 4; }
            if cell.underline { flags |= 8; }
            if cell.inverse { flags |= 16; }
            if cell.wide { flags |= 32; }
            if cell.wide_continuation { flags |= 64; }
            buf.push(flags);
            // 4 bytes padding for alignment
            buf.extend_from_slice(&[0u8; 4]);
        }
    }

    buf
}

fn decode_binary(data: &[u8]) -> (u16, u16, u16, u16, usize) {
    let rows = u16::from_le_bytes([data[0], data[1]]);
    let cols = u16::from_le_bytes([data[2], data[3]]);
    let cursor_row = u16::from_le_bytes([data[4], data[5]]);
    let cursor_col = u16::from_le_bytes([data[6], data[7]]);
    let cell_count = rows as usize * cols as usize;
    (rows, cols, cursor_row, cursor_col, cell_count)
}

fn parse_hex_color(s: &str) -> (u8, u8, u8) {
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (0, 0, 0)
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_json_serialization(c: &mut Criterion) {
    let snap_30x120 = make_snapshot(30, 120);
    let snap_50x120 = make_snapshot(50, 120);

    let json_30 = serde_json::to_vec(&snap_30x120).unwrap();
    let json_50 = serde_json::to_vec(&snap_50x120).unwrap();

    let mut group = c.benchmark_group("richgrid_json_serialization");

    // --- Serialization ---

    group.throughput(Throughput::Elements(30 * 120));
    group.bench_with_input(
        BenchmarkId::new("serialize_30x120", "30x120"),
        &snap_30x120,
        |b, snap| {
            b.iter(|| serde_json::to_vec(snap).unwrap());
        },
    );

    group.throughput(Throughput::Elements(50 * 120));
    group.bench_with_input(
        BenchmarkId::new("serialize_50x120", "50x120"),
        &snap_50x120,
        |b, snap| {
            b.iter(|| serde_json::to_vec(snap).unwrap());
        },
    );

    // --- Deserialization ---

    group.throughput(Throughput::Elements(30 * 120));
    group.bench_with_input(
        BenchmarkId::new("deserialize_30x120", "30x120"),
        &json_30,
        |b, data| {
            b.iter(|| serde_json::from_slice::<RichGridData>(data).unwrap());
        },
    );

    group.throughput(Throughput::Elements(50 * 120));
    group.bench_with_input(
        BenchmarkId::new("deserialize_50x120", "50x120"),
        &json_50,
        |b, data| {
            b.iter(|| serde_json::from_slice::<RichGridData>(data).unwrap());
        },
    );

    group.finish();
}

fn bench_json_payload_size(c: &mut Criterion) {
    // This benchmark doesn't measure speed, but we use criterion to report
    // the payload sizes as a reference for optimization decisions.
    let snap = make_snapshot(30, 120);
    let json_bytes = serde_json::to_vec(&snap).unwrap();
    let binary_bytes = encode_binary(&snap);

    let mut group = c.benchmark_group("richgrid_payload_size");

    // Measure serialization throughput normalized by byte output size
    group.throughput(Throughput::Bytes(json_bytes.len() as u64));
    group.bench_function("json_30x120_bytes", |b| {
        b.iter(|| serde_json::to_vec(&snap).unwrap());
    });

    group.throughput(Throughput::Bytes(binary_bytes.len() as u64));
    group.bench_function("binary_30x120_bytes", |b| {
        b.iter(|| encode_binary(&snap));
    });

    group.finish();
}

fn bench_binary_comparison(c: &mut Criterion) {
    let snap_30x120 = make_snapshot(30, 120);
    let binary_30 = encode_binary(&snap_30x120);

    let mut group = c.benchmark_group("richgrid_binary_comparison");

    // --- Binary encode ---
    group.throughput(Throughput::Elements(30 * 120));
    group.bench_with_input(
        BenchmarkId::new("binary_encode_30x120", "30x120"),
        &snap_30x120,
        |b, snap| {
            b.iter(|| encode_binary(snap));
        },
    );

    // --- Binary decode (header only, to show parsing floor) ---
    group.throughput(Throughput::Elements(30 * 120));
    group.bench_with_input(
        BenchmarkId::new("binary_decode_header_30x120", "30x120"),
        &binary_30,
        |b, data| {
            b.iter(|| decode_binary(data));
        },
    );

    // --- JSON serialize for direct comparison ---
    group.throughput(Throughput::Elements(30 * 120));
    group.bench_with_input(
        BenchmarkId::new("json_serialize_30x120", "30x120"),
        &snap_30x120,
        |b, snap| {
            b.iter(|| serde_json::to_vec(snap).unwrap());
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_json_serialization,
    bench_json_payload_size,
    bench_binary_comparison,
);
criterion_main!(benches);
