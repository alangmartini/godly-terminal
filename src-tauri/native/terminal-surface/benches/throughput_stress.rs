use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use godly_protocol::types::{
    CursorShape, CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
};
use godly_terminal_surface::font_metrics::FontMetrics;
use godly_terminal_surface::glyph_cache::GlyphCache;
use godly_terminal_surface::glyph_rasterizer::{
    GlyphRasterizer, MeasuredFontMetrics, RasterizedGlyph,
};
use godly_terminal_surface::pixel_renderer::PixelRenderer;
use iced::Color;

// ---------------------------------------------------------------------------
// Stub rasterizer
// ---------------------------------------------------------------------------

struct StubRasterizer;

impl GlyphRasterizer for StubRasterizer {
    fn rasterize(
        &mut self,
        _ch: char,
        _font_size_px: f32,
        _bold: bool,
        _italic: bool,
    ) -> Option<RasterizedGlyph> {
        Some(RasterizedGlyph {
            alpha: vec![128; 4], // 2x2 alpha mask
            width: 2,
            height: 2,
            bearing_x: 0,
            bearing_y: 2,
            advance: 8.0,
        })
    }

    fn measure(&mut self, font_size_px: f32) -> MeasuredFontMetrics {
        MeasuredFontMetrics {
            ascent: font_size_px * 0.8,
            descent: font_size_px * 0.2,
            leading: 0.0,
            average_advance: font_size_px * 0.6,
            is_monospace: true,
        }
    }

    fn has_glyph(&self, _ch: char) -> bool {
        true
    }

    fn load_font(&mut self, _data: &[u8], _index: u32) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Grid fixture helpers
// ---------------------------------------------------------------------------

const PALETTE: &[&str] = &[
    "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd",
];
const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

fn make_cell(row: usize, col: usize) -> RichGridCell {
    let ci = (row.wrapping_mul(131) + col.wrapping_mul(37)) % CHARS.len();
    let fi = (row.wrapping_mul(7) + col.wrapping_mul(13)) % PALETTE.len();
    let bi = (row.wrapping_mul(3) + col.wrapping_mul(11)) % PALETTE.len();
    RichGridCell {
        content: String::from(CHARS[ci] as char),
        fg: PALETTE[fi].into(),
        bg: if bi == 0 {
            "default".into()
        } else {
            PALETTE[bi].into()
        },
        bold: col % 17 == 0,
        dim: col % 23 == 0,
        italic: col % 19 == 0,
        underline: col % 29 == 0,
        inverse: col % 31 == 0,
        wide: false,
        wide_continuation: false,
    }
}

fn empty_cell() -> RichGridCell {
    RichGridCell {
        content: " ".to_string(),
        fg: "default".to_string(),
        bg: "default".to_string(),
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        inverse: false,
        wide: false,
        wide_continuation: false,
    }
}

fn wrap_grid(rows: u16, cols: u16, grid_rows: Vec<RichGridRow>) -> RichGridData {
    RichGridData {
        rows: grid_rows,
        cursor: CursorState {
            row: 0,
            col: 0,
            cursor_style: CursorShape::SteadyBlock,
        },
        dimensions: GridDimensions { rows, cols },
        alternate_screen: false,
        cursor_hidden: false,
        title: String::new(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

fn make_grid(rows: u16, cols: u16) -> RichGridData {
    make_grid_with_dirty_pct(rows, cols, 1.0)
}

fn make_grid_with_dirty_pct(rows: u16, cols: u16, dirty_pct: f32) -> RichGridData {
    let dirty_rows = (rows as f32 * dirty_pct).ceil() as usize;
    let grid_rows = (0..rows as usize)
        .map(|r| RichGridRow {
            cells: if r < dirty_rows {
                (0..cols as usize).map(|c| make_cell(r, c)).collect()
            } else {
                (0..cols as usize).map(|_| empty_cell()).collect()
            },
            wrapped: false,
        })
        .collect();
    wrap_grid(rows, cols, grid_rows)
}

fn make_grid_with_seed(rows: u16, cols: u16, seed: usize) -> RichGridData {
    let grid_rows = (0..rows as usize)
        .map(|r| RichGridRow {
            cells: (0..cols as usize)
                .map(|c| make_cell(r.wrapping_add(seed), c.wrapping_add(seed)))
                .collect(),
            wrapped: false,
        })
        .collect();
    wrap_grid(rows, cols, grid_rows)
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Sustained throughput: render the same 80x24 grid 100 times in a loop.
fn bench_sustained(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_throughput_sustained");
    let grid = make_grid(24, 80);
    let metrics = FontMetrics::from_font_size(14.0);

    // Pre-warm renderer and cache
    let mut renderer = PixelRenderer::new();
    let mut cache = GlyphCache::new();
    let mut rasterizer = StubRasterizer;
    renderer.render(
        &grid,
        &metrics,
        &mut cache,
        &mut rasterizer,
        Color::WHITE,
        Color::BLACK,
        None,
    );

    group.throughput(Throughput::Elements(100));
    group.bench_function("80x24_x100", |b| {
        b.iter(|| {
            for _ in 0..100 {
                renderer.render(
                    &grid,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
            }
        });
    });
    group.finish();
}

/// Varying grids: render 100 different grids simulating rapidly changing output.
/// Stresses the glyph cache with more unique character/style combinations.
fn bench_varying_grids(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_throughput_varying_grids");
    let metrics = FontMetrics::from_font_size(14.0);

    for &(rows, cols, label) in &[(24u16, 80u16, "80x24"), (40, 120, "120x40")] {
        let grids: Vec<RichGridData> = (0..100)
            .map(|i| make_grid_with_seed(rows, cols, i))
            .collect();

        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rasterizer = StubRasterizer;
        // Pre-warm
        renderer.render(
            &grids[0],
            &metrics,
            &mut cache,
            &mut rasterizer,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        group.throughput(Throughput::Elements(100));
        group.bench_with_input(
            BenchmarkId::new("varying_x100", label),
            &grids,
            |b, grids| {
                b.iter(|| {
                    for grid in grids {
                        renderer.render(
                            grid,
                            &metrics,
                            &mut cache,
                            &mut rasterizer,
                            Color::WHITE,
                            Color::BLACK,
                            None,
                        );
                    }
                });
            },
        );
    }

    group.finish();
}

/// Buffer reuse efficiency: compare same-size vs different-size consecutive renders.
/// Measures the overhead of buffer reallocation when grid dimensions change.
fn bench_buffer_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_reuse_efficiency");
    let metrics = FontMetrics::from_font_size(14.0);

    let grid_a = make_grid(24, 80);
    let grid_b = make_grid(24, 80); // same dimensions as A
    let grid_large = make_grid(40, 120); // different dimensions

    // Same-size: render A then B (buffer reused, no realloc)
    {
        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rasterizer = StubRasterizer;
        renderer.render(
            &grid_a,
            &metrics,
            &mut cache,
            &mut rasterizer,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        group.bench_function("same_size", |b| {
            b.iter(|| {
                renderer.render(
                    &grid_a,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
                renderer.render(
                    &grid_b,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
            });
        });
    }

    // Different-size: render A (80x24) then large (120x40) — buffer resized
    {
        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rasterizer = StubRasterizer;
        renderer.render(
            &grid_a,
            &metrics,
            &mut cache,
            &mut rasterizer,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        group.bench_function("different_size", |b| {
            b.iter(|| {
                renderer.render(
                    &grid_a,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
                renderer.render(
                    &grid_large,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
            });
        });
    }

    group.finish();
}

/// Dirty row percentage: vary content density to measure how rendering time
/// scales with the proportion of non-empty rows.
fn bench_dirty_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_row_percentage");
    let metrics = FontMetrics::from_font_size(14.0);

    for &(pct, label) in &[(0.1f32, "10pct"), (0.5, "50pct"), (1.0, "100pct")] {
        let grid = make_grid_with_dirty_pct(24, 80, pct);

        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rasterizer = StubRasterizer;
        // Pre-warm
        renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rasterizer,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        group.bench_with_input(BenchmarkId::new("80x24", label), &grid, |b, grid| {
            b.iter(|| {
                renderer.render(
                    grid,
                    &metrics,
                    &mut cache,
                    &mut rasterizer,
                    Color::WHITE,
                    Color::BLACK,
                    None,
                );
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sustained,
    bench_varying_grids,
    bench_buffer_reuse,
    bench_dirty_rows,
);
criterion_main!(benches);
