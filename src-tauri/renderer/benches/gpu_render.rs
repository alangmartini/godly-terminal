use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use godly_protocol::types::{
    CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
};
use godly_renderer::GpuRenderer;

// ---------------------------------------------------------------------------
// Fixture builders (same palette as protocol benchmarks for consistency)
// ---------------------------------------------------------------------------

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

/// Sparse grid: mostly empty cells with occasional colored text.
/// Simulates a typical shell session (prompt + sparse output).
fn make_sparse_snapshot(rows: u16, cols: u16) -> RichGridData {
    let grid_rows = (0..rows as usize)
        .map(|r| {
            let cells = (0..cols as usize)
                .map(|c| {
                    // ~20% of cells have content, rest are empty
                    if (r * 131 + c * 37) % 5 == 0 {
                        make_cell(r, c)
                    } else {
                        RichGridCell {
                            content: " ".into(),
                            fg: "default".into(),
                            bg: "default".into(),
                            bold: false,
                            dim: false,
                            italic: false,
                            underline: false,
                            inverse: false,
                            wide: false,
                            wide_continuation: false,
                        }
                    }
                })
                .collect();
            RichGridRow {
                cells,
                wrapped: false,
            }
        })
        .collect();

    RichGridData {
        rows: grid_rows,
        cursor: CursorState { row: rows - 1, col: 2 },
        dimensions: GridDimensions { rows, cols },
        alternate_screen: false,
        cursor_hidden: false,
        title: "bash".into(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

/// Dense colored grid: every cell has unique colors and attributes.
/// Simulates heavy TUI output (htop, cargo build with colors).
fn make_dense_snapshot(rows: u16, cols: u16) -> RichGridData {
    let grid_rows = (0..rows as usize)
        .map(|r| {
            let cells = (0..cols as usize)
                .map(|c| {
                    let ci = (r * 131 + c * 37) % CHARS.len();
                    let fi = (r * 7 + c * 13) % PALETTE.len();
                    let bi = (r * 3 + c * 11) % PALETTE.len();
                    RichGridCell {
                        content: String::from(CHARS[ci] as char),
                        fg: PALETTE[fi].into(),
                        bg: PALETTE[bi].into(),
                        bold: c % 3 == 0,
                        dim: c % 7 == 0,
                        italic: c % 5 == 0,
                        underline: c % 4 == 0,
                        inverse: c % 11 == 0,
                        wide: false,
                        wide_continuation: false,
                    }
                })
                .collect();
            RichGridRow {
                cells,
                wrapped: r % 3 == 0,
            }
        })
        .collect();

    RichGridData {
        rows: grid_rows,
        cursor: CursorState { row: rows - 1, col: 0 },
        dimensions: GridDimensions { rows, cols },
        alternate_screen: false,
        cursor_hidden: false,
        title: "cargo build".into(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark render_to_pixels across grid sizes.
/// This is the hot path: daemon snapshot → GPU render → raw RGBA buffer.
fn bench_render_to_pixels(c: &mut Criterion) {
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };

    let mut group = c.benchmark_group("render_to_pixels");

    let sizes: &[(u16, u16)] = &[
        (24, 80),   // standard terminal
        (30, 120),  // typical modern
        (50, 120),  // large terminal
        (50, 200),  // ultrawide
    ];

    for &(rows, cols) in sizes {
        let snapshot = make_snapshot(rows, cols);
        let total_cells = rows as u64 * cols as u64;
        group.throughput(Throughput::Elements(total_cells));
        group.bench_with_input(
            BenchmarkId::new("mixed", format!("{rows}x{cols}")),
            &snapshot,
            |b, snap| {
                b.iter(|| renderer.render_to_pixels(snap).unwrap());
            },
        );
    }

    group.finish();
}

/// Benchmark render_to_png (includes PNG encoding overhead).
/// Compares against render_to_pixels to isolate PNG cost.
fn bench_render_to_png(c: &mut Criterion) {
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };

    let mut group = c.benchmark_group("render_to_png");

    let sizes: &[(u16, u16)] = &[
        (24, 80),
        (50, 120),
    ];

    for &(rows, cols) in sizes {
        let snapshot = make_snapshot(rows, cols);
        let total_cells = rows as u64 * cols as u64;
        group.throughput(Throughput::Elements(total_cells));
        group.bench_with_input(
            BenchmarkId::new("mixed", format!("{rows}x{cols}")),
            &snapshot,
            |b, snap| {
                b.iter(|| renderer.render_to_png(snap).unwrap());
            },
        );
    }

    group.finish();
}

/// Benchmark with different content patterns.
/// Same grid size (30x120) but different data density.
fn bench_content_patterns(c: &mut Criterion) {
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };

    let rows: u16 = 30;
    let cols: u16 = 120;
    let total_cells = rows as u64 * cols as u64;

    let sparse = make_sparse_snapshot(rows, cols);
    let dense = make_dense_snapshot(rows, cols);
    let mixed = make_snapshot(rows, cols);

    let mut group = c.benchmark_group("content_patterns/30x120");
    group.throughput(Throughput::Elements(total_cells));

    group.bench_function("sparse", |b| {
        b.iter(|| renderer.render_to_pixels(&sparse).unwrap());
    });

    group.bench_function("dense", |b| {
        b.iter(|| renderer.render_to_pixels(&dense).unwrap());
    });

    group.bench_function("mixed", |b| {
        b.iter(|| renderer.render_to_pixels(&mixed).unwrap());
    });

    group.finish();
}

/// Benchmark repeated renders (warm atlas — all glyphs cached).
/// First render populates the glyph atlas; subsequent renders reuse it.
/// Measures steady-state rendering performance.
fn bench_warm_vs_cold(c: &mut Criterion) {
    let snapshot = make_snapshot(30, 120);

    let mut group = c.benchmark_group("warm_atlas");

    // Cold: new renderer for each iteration (atlas empty)
    group.bench_function("cold_30x120", |b| {
        b.iter(|| {
            let mut renderer = GpuRenderer::new("Cascadia Code", 14.0).unwrap();
            renderer.render_to_pixels(&snapshot).unwrap()
        });
    });

    // Warm: reuse renderer (atlas pre-populated)
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };
    // Pre-warm the atlas
    let _ = renderer.render_to_pixels(&snapshot);

    group.bench_function("warm_30x120", |b| {
        b.iter(|| renderer.render_to_pixels(&snapshot).unwrap());
    });

    group.finish();
}

/// Benchmark the full pipeline: render_to_pixels + pack into raw RGBA wire format.
/// This is the actual code path used by the gpuframe:// protocol with ?format=raw.
fn bench_full_pipeline(c: &mut Criterion) {
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };

    // Pre-warm
    let snapshot = make_snapshot(30, 120);
    let _ = renderer.render_to_pixels(&snapshot);

    let mut group = c.benchmark_group("full_pipeline");

    group.bench_function("pixels_30x120", |b| {
        b.iter(|| renderer.render_to_pixels(&snapshot).unwrap());
    });

    group.bench_function("png_30x120", |b| {
        b.iter(|| renderer.render_to_png(&snapshot).unwrap());
    });

    group.bench_function("raw_wire_30x120", |b| {
        b.iter(|| {
            let (width, height, pixels) = renderer.render_to_pixels(&snapshot).unwrap();
            let mut result = Vec::with_capacity(8 + pixels.len());
            result.extend_from_slice(&width.to_le_bytes());
            result.extend_from_slice(&height.to_le_bytes());
            result.extend_from_slice(&pixels);
            result
        });
    });

    group.finish();
}

/// Benchmark first-render latency when the renderer was pre-warmed.
/// Simulates the app startup path: GpuRenderer::new() runs on a background
/// thread, then the first actual render request hits an already-initialized renderer.
fn bench_prewarmed_first_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("prewarmed");

    // Pre-warm: create renderer once (simulates background thread completion)
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU benchmarks: {e}");
            return;
        }
    };

    // First render on a pre-warmed renderer (atlas populated, GPU device ready)
    let snapshot = make_snapshot(30, 120);
    group.bench_function("first_render_30x120", |b| {
        b.iter(|| renderer.render_to_pixels(&snapshot).unwrap());
    });

    group.finish();
}

/// Benchmark individual cold-start phases to isolate the bottleneck.
/// This breaks `GpuRenderer::new()` into its constituent parts:
///   1. GPU device initialization (wgpu adapter + device)
///   2. GlyphAtlas construction (FontSystem + ASCII pre-rasterization)
///   3. RenderPipeline creation (WGSL shader compilation)
fn bench_cold_start_phases(c: &mut Criterion) {
    use godly_renderer::cold_start_phases;

    let mut group = c.benchmark_group("cold_start_phases");

    // Phase 1: GPU device creation
    group.bench_function("gpu_device", |b| {
        b.iter(|| cold_start_phases::create_device().unwrap());
    });

    // Phase 2: Glyph atlas (FontSystem + pre-rasterize ASCII)
    group.bench_function("glyph_atlas", |b| {
        b.iter(|| cold_start_phases::create_atlas("Cascadia Code", 14.0));
    });

    // Phase 3: Render pipeline (shader compile)
    // Need a device for this
    let dev = cold_start_phases::create_device().unwrap();
    group.bench_function("render_pipeline", |b| {
        b.iter(|| cold_start_phases::create_pipeline(&dev));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_render_to_pixels,
    bench_render_to_png,
    bench_content_patterns,
    bench_warm_vs_cold,
    bench_full_pipeline,
    bench_prewarmed_first_render,
    bench_cold_start_phases,
);
criterion_main!(benches);
