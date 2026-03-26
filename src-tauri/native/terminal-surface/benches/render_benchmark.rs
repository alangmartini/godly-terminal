use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use godly_protocol::types::{
    CursorShape, CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
};
use godly_terminal_surface::font_metrics::FontMetrics;
use godly_terminal_surface::glyph_cache::{CachedGlyph, GlyphCache, GlyphKey};
use godly_terminal_surface::glyph_rasterizer::{
    GlyphFormat, GlyphRasterizer, MeasuredFontMetrics, RasterizedGlyph,
};
use godly_terminal_surface::pixel_renderer::PixelRenderer;
use godly_terminal_surface::GridPos;
use iced::Color;

// ---------------------------------------------------------------------------
// Stub rasterizer (returns 2x2 alpha mask for every glyph)
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
            data: vec![128; 4],
            format: GlyphFormat::Alpha,
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
    RichGridCell {
        content: String::from((CHARS[(row * 7 + col) % CHARS.len()] as char)),
        fg: PALETTE[(row + col) % PALETTE.len()].to_string(),
        bg: if (row + col) % 5 == 0 {
            "#1e1e2e".to_string()
        } else {
            "default".to_string()
        },
        bold: col % 7 == 0,
        dim: col % 11 == 0,
        italic: col % 13 == 0,
        underline: col % 17 == 0,
        inverse: false,
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

fn make_grid_data(rows: u16, cols: u16, cell_fn: fn(usize, usize) -> RichGridCell) -> RichGridData {
    RichGridData {
        rows: (0..rows as usize)
            .map(|r| RichGridRow {
                cells: (0..cols as usize).map(|c| cell_fn(r, c)).collect(),
                wrapped: false,
            })
            .collect(),
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
    make_grid_data(rows, cols, make_cell)
}

fn make_sparse_grid(rows: u16, cols: u16) -> RichGridData {
    // Can't use a closure as fn pointer when it captures `cols`, so
    // build the sparse pattern inline via make_grid_data's approach.
    RichGridData {
        rows: (0..rows as usize)
            .map(|r| RichGridRow {
                cells: (0..cols as usize)
                    .map(|c| {
                        if (r * cols as usize + c) % 10 == 0 {
                            make_cell(r, c)
                        } else {
                            empty_cell()
                        }
                    })
                    .collect(),
                wrapped: false,
            })
            .collect(),
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

fn stub_glyph() -> CachedGlyph {
    CachedGlyph {
        data: vec![128; 4],
        format: GlyphFormat::Alpha,
        width: 2,
        height: 2,
        bearing_x: 0,
        bearing_y: 2,
        advance: 8.0,
    }
}

/// Render once to warm the glyph cache, then return the components for benchmarking.
fn warm_render_state(
    grid: &RichGridData,
    metrics: &FontMetrics,
    selection: Option<(GridPos, GridPos)>,
) -> (PixelRenderer, GlyphCache, StubRasterizer) {
    let mut renderer = PixelRenderer::new();
    let mut cache = GlyphCache::new();
    let mut rast = StubRasterizer;
    renderer.render(
        grid, metrics, &mut cache, &mut rast,
        Color::WHITE, Color::BLACK, selection,
    );
    (renderer, cache, rast)
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_grid_sizes(c: &mut Criterion) {
    let metrics = FontMetrics::from_font_size(14.0);
    let sizes: &[(u16, u16)] = &[(80, 24), (120, 40), (200, 50), (300, 80)];

    let mut group = c.benchmark_group("pixel_render_grid_sizes");

    for &(cols, rows) in sizes {
        let grid = make_grid(rows, cols);
        let label = format!("{}x{}", cols, rows);

        group.throughput(Throughput::Elements((rows as u64) * (cols as u64)));
        group.bench_with_input(BenchmarkId::new(&label, &label), &grid, |b, grid| {
            let (mut renderer, mut cache, mut rast) = warm_render_state(grid, &metrics, None);

            b.iter(|| {
                let (_, w, h) = renderer.render(
                    grid, &metrics, &mut cache, &mut rast,
                    Color::WHITE, Color::BLACK, None,
                );
                (w, h)
            });
        });
    }

    group.finish();
}

fn bench_cold_vs_warm(c: &mut Criterion) {
    let metrics = FontMetrics::from_font_size(14.0);
    let grid = make_grid(24, 80);

    let mut group = c.benchmark_group("pixel_render_cold_vs_warm");
    group.throughput(Throughput::Elements(24 * 80));

    group.bench_function("cold_cache", |b| {
        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;

        b.iter(|| {
            cache.invalidate();
            let (_, w, h) = renderer.render(
                &grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, None,
            );
            (w, h)
        });
    });

    group.bench_function("warm_cache", |b| {
        let (mut renderer, mut cache, mut rast) = warm_render_state(&grid, &metrics, None);

        b.iter(|| {
            let (_, w, h) = renderer.render(
                &grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, None,
            );
            (w, h)
        });
    });

    group.finish();
}

fn bench_with_selection(c: &mut Criterion) {
    let metrics = FontMetrics::from_font_size(14.0);
    let grid = make_grid(24, 80);
    let full_selection = Some((
        GridPos { row: 0, col: 0 },
        GridPos { row: 23, col: 79 },
    ));

    let mut group = c.benchmark_group("pixel_render_with_selection");
    group.throughput(Throughput::Elements(24 * 80));

    group.bench_function("no_selection", |b| {
        let (mut renderer, mut cache, mut rast) = warm_render_state(&grid, &metrics, None);

        b.iter(|| {
            let (_, w, h) = renderer.render(
                &grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, None,
            );
            (w, h)
        });
    });

    group.bench_function("full_selection", |b| {
        let (mut renderer, mut cache, mut rast) =
            warm_render_state(&grid, &metrics, full_selection);

        b.iter(|| {
            let (_, w, h) = renderer.render(
                &grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, full_selection,
            );
            (w, h)
        });
    });

    group.finish();
}

fn bench_dense_vs_sparse(c: &mut Criterion) {
    let metrics = FontMetrics::from_font_size(14.0);
    let dense_grid = make_grid(24, 80);
    let sparse_grid = make_sparse_grid(24, 80);

    let mut group = c.benchmark_group("pixel_render_dense_vs_sparse");
    group.throughput(Throughput::Elements(24 * 80));

    group.bench_function("dense", |b| {
        let (mut renderer, mut cache, mut rast) = warm_render_state(&dense_grid, &metrics, None);

        b.iter(|| {
            let (_, w, h) = renderer.render(
                &dense_grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, None,
            );
            (w, h)
        });
    });

    group.bench_function("sparse", |b| {
        let (mut renderer, mut cache, mut rast) = warm_render_state(&sparse_grid, &metrics, None);

        b.iter(|| {
            let (_, w, h) = renderer.render(
                &sparse_grid, &metrics, &mut cache, &mut rast,
                Color::WHITE, Color::BLACK, None,
            );
            (w, h)
        });
    });

    group.finish();
}

fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("glyph_cache_operations");

    group.bench_function("cache_insert", |b| {
        let mut cache = GlyphCache::new();
        let mut i = 0u32;

        b.iter(|| {
            let ch = char::from_u32(('A' as u32) + (i % 26)).unwrap();
            let key = GlyphKey::new(ch, 14.0, i % 2 == 0, i % 3 == 0);
            cache.insert(key, stub_glyph());
            i = i.wrapping_add(1);
        });
    });

    group.bench_function("cache_get_hit", |b| {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        cache.insert(key, stub_glyph());

        b.iter(|| cache.get(&key));
    });

    group.bench_function("cache_get_miss", |b| {
        let cache = GlyphCache::new();
        let key = GlyphKey::new('Z', 14.0, false, false);

        b.iter(|| cache.get(&key));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_grid_sizes,
    bench_cold_vs_warm,
    bench_with_selection,
    bench_dense_vs_sparse,
    bench_cache_operations,
);
criterion_main!(benches);
