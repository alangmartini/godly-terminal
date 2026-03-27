use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use godly_protocol::types::{
    CursorShape, CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
};
use godly_terminal_surface::font_metrics::FontMetrics;
use godly_terminal_surface::glyph_cache::GlyphCache;
use godly_terminal_surface::glyph_rasterizer::{
    GlyphFormat, GlyphRasterizer, MeasuredFontMetrics, RasterizedGlyph,
};
use godly_terminal_surface::pixel_renderer::PixelRenderer;
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
// Color conversion (mirrors daemon/src/session.rs:1767+)
// ---------------------------------------------------------------------------

fn color_to_hex(color: godly_vt::Color) -> String {
    match color {
        godly_vt::Color::Default => "default".to_string(),
        godly_vt::Color::Idx(idx) => match idx {
            0 => "#000000".to_string(),
            1 => "#cd3131".to_string(),
            2 => "#0dbc79".to_string(),
            3 => "#e5e510".to_string(),
            4 => "#2472c8".to_string(),
            5 => "#bc3fbc".to_string(),
            6 => "#11a8cd".to_string(),
            7 => "#e5e5e5".to_string(),
            8 => "#666666".to_string(),
            9 => "#f14c4c".to_string(),
            10 => "#23d18b".to_string(),
            11 => "#f5f543".to_string(),
            12 => "#3b8eea".to_string(),
            13 => "#d670d6".to_string(),
            14 => "#29b8db".to_string(),
            15 => "#e5e5e5".to_string(),
            16..=231 => {
                let i = idx - 16;
                let r = i / 36;
                let g = (i % 36) / 6;
                let b = i % 6;
                let r = if r == 0 { 0 } else { 55 + 40 * r };
                let g = if g == 0 { 0 } else { 55 + 40 * g };
                let b = if b == 0 { 0 } else { 55 + 40 * b };
                format!("#{:02x}{:02x}{:02x}", r, g, b)
            }
            232..=255 => {
                let v = 8 + 10 * (idx - 232);
                format!("#{:02x}{:02x}{:02x}", v, v, v)
            }
        },
        godly_vt::Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
    }
}

// ---------------------------------------------------------------------------
// VT screen -> RichGridData conversion (mirrors daemon/src/session.rs:1282-1345)
// ---------------------------------------------------------------------------

fn vt_cursor_to_protocol(style: godly_vt::CursorStyle) -> CursorShape {
    match style {
        godly_vt::CursorStyle::BlinkBlock => CursorShape::BlinkBlock,
        godly_vt::CursorStyle::SteadyBlock => CursorShape::SteadyBlock,
        godly_vt::CursorStyle::BlinkUnderline => CursorShape::BlinkUnderline,
        godly_vt::CursorStyle::SteadyUnderline => CursorShape::SteadyUnderline,
        godly_vt::CursorStyle::BlinkBar => CursorShape::BlinkBar,
        godly_vt::CursorStyle::SteadyBar => CursorShape::SteadyBar,
    }
}

fn vt_to_rich_grid(screen: &godly_vt::Screen) -> RichGridData {
    let (num_rows, cols) = screen.size();
    let (cursor_row, cursor_col) = screen.cursor_position();

    let mut rows = Vec::with_capacity(usize::from(num_rows));
    for row_idx in 0..num_rows {
        let mut cells = Vec::with_capacity(usize::from(cols));
        for col_idx in 0..cols {
            let cell = screen.cell(row_idx, col_idx);
            match cell {
                Some(c) => {
                    cells.push(RichGridCell {
                        content: c.contents().to_string(),
                        fg: color_to_hex(c.fgcolor()),
                        bg: color_to_hex(c.bgcolor()),
                        bold: c.bold(),
                        dim: c.dim(),
                        italic: c.italic(),
                        underline: c.underline(),
                        inverse: c.inverse(),
                        wide: c.is_wide(),
                        wide_continuation: c.is_wide_continuation(),
                    });
                }
                None => {
                    cells.push(RichGridCell {
                        content: String::new(),
                        fg: "default".to_string(),
                        bg: "default".to_string(),
                        bold: false,
                        dim: false,
                        italic: false,
                        underline: false,
                        inverse: false,
                        wide: false,
                        wide_continuation: false,
                    });
                }
            }
        }
        let wrapped = screen.row_wrapped(row_idx);
        rows.push(RichGridRow { cells, wrapped });
    }

    RichGridData {
        rows,
        cursor: CursorState {
            row: cursor_row,
            col: cursor_col,
            cursor_style: vt_cursor_to_protocol(screen.cursor_style()),
        },
        dimensions: GridDimensions {
            rows: num_rows,
            cols,
        },
        alternate_screen: screen.alternate_screen(),
        cursor_hidden: screen.hide_cursor(),
        title: screen.window_title().to_string(),
        scrollback_offset: screen.scrollback(),
        total_scrollback: screen.scrollback_count(),
    }
}

// ---------------------------------------------------------------------------
// Data generators
// ---------------------------------------------------------------------------

/// Simulated `cargo build` output with ANSI colors, ~1MB.
fn gen_mixed_realistic() -> Vec<u8> {
    let lines = [
        "\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m godly-vt v0.1.0 (C:\\Users\\dev\\godly-terminal\\src-tauri\\godly-vt)\n",
        "\x1b[0m\x1b[1m\x1b[33mwarning\x1b[0m: unused variable: `threshold`\n",
        " \x1b[0m\x1b[1m\x1b[34m-->\x1b[0m godly-vt\\src\\simd\\sse2.rs:20:13\n",
        "   \x1b[0m\x1b[1m\x1b[34m|\x1b[0m\n",
        "\x1b[1m\x1b[34m20\x1b[0m \x1b[0m\x1b[1m\x1b[34m|\x1b[0m         let threshold = _mm_set1_epi8(0x20);\n",
        "   \x1b[0m\x1b[1m\x1b[34m|\x1b[0m             \x1b[0m\x1b[1m\x1b[33m^^^^^^^^^\x1b[0m \x1b[0m\x1b[1m\x1b[33mhelp: prefix with underscore\x1b[0m\n",
        "\x1b[0m\x1b[1m\x1b[32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 2.34s\n",
    ];
    let mut data = Vec::with_capacity(1024 * 1024);
    while data.len() < 1024 * 1024 {
        for line in &lines {
            data.extend_from_slice(line.as_bytes());
        }
    }
    data.truncate(1024 * 1024);
    data
}

/// ~1KB of colored text lines for incremental benchmark.
fn gen_small_update() -> Vec<u8> {
    let lines = [
        "\x1b[32mOK\x1b[0m test_parse_basic ... passed\n",
        "\x1b[31mFAIL\x1b[0m test_edge_case ... assertion failed\n",
        "\x1b[33mWARN\x1b[0m: deprecation notice for foo()\n",
    ];
    let mut data = Vec::with_capacity(1024);
    while data.len() < 1024 {
        for line in &lines {
            data.extend_from_slice(line.as_bytes());
        }
    }
    data.truncate(1024);
    data
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

/// Full end-to-end pipeline at standard 80x24 terminal size:
/// VT parse -> grid conversion -> pixel render.
fn bench_pipeline_80x24(c: &mut Criterion) {
    let data = gen_mixed_realistic();
    let metrics = FontMetrics::from_font_size(14.0);
    let rows: u64 = 24;
    let cols: u64 = 80;

    let mut group = c.benchmark_group("pipeline_80x24");
    group.throughput(Throughput::Elements(rows * cols));

    group.bench_function("full", |b| {
        b.iter(|| {
            let mut parser = godly_vt::Parser::new(24, 80, 0);
            parser.process(&data);
            let grid = vt_to_rich_grid(parser.screen());

            let mut renderer = PixelRenderer::new();
            let mut cache = GlyphCache::new();
            let mut rast = StubRasterizer;
            let (_, w, h) = renderer.render(
                &grid,
                &metrics,
                &mut cache,
                &mut rast,
                Color::WHITE,
                Color::BLACK,
                None,
            );
            (w, h)
        });
    });

    group.finish();
}

/// Full end-to-end pipeline at larger 120x40 terminal size.
fn bench_pipeline_120x40(c: &mut Criterion) {
    let data = gen_mixed_realistic();
    let metrics = FontMetrics::from_font_size(14.0);
    let rows: u64 = 40;
    let cols: u64 = 120;

    let mut group = c.benchmark_group("pipeline_120x40");
    group.throughput(Throughput::Elements(rows * cols));

    group.bench_function("full", |b| {
        b.iter(|| {
            let mut parser = godly_vt::Parser::new(40, 120, 0);
            parser.process(&data);
            let grid = vt_to_rich_grid(parser.screen());

            let mut renderer = PixelRenderer::new();
            let mut cache = GlyphCache::new();
            let mut rast = StubRasterizer;
            let (_, w, h) = renderer.render(
                &grid,
                &metrics,
                &mut cache,
                &mut rast,
                Color::WHITE,
                Color::BLACK,
                None,
            );
            (w, h)
        });
    });

    group.finish();
}

/// Incremental pipeline: pre-fill parser, then benchmark parsing a small
/// 1KB chunk + convert + render. Simulates per-frame cost during normal use.
fn bench_pipeline_incremental(c: &mut Criterion) {
    let initial_data = gen_mixed_realistic();
    let update_data = gen_small_update();
    let metrics = FontMetrics::from_font_size(14.0);
    let rows: u64 = 24;
    let cols: u64 = 80;

    let mut group = c.benchmark_group("pipeline_incremental");
    group.throughput(Throughput::Elements(rows * cols));

    group.bench_function("small_update", |b| {
        let mut parser = godly_vt::Parser::new(24, 80, 0);
        parser.process(&initial_data);

        let mut renderer = PixelRenderer::new();
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;

        // Warm the glyph cache so incremental iterations measure steady-state
        let grid = vt_to_rich_grid(parser.screen());
        let _ = renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        b.iter(|| {
            parser.process(&update_data);
            let grid = vt_to_rich_grid(parser.screen());
            let (_, w, h) = renderer.render(
                &grid,
                &metrics,
                &mut cache,
                &mut rast,
                Color::WHITE,
                Color::BLACK,
                None,
            );
            (w, h)
        });
    });

    group.finish();
}

/// Isolate the screen -> RichGridData conversion to understand its cost
/// relative to the full pipeline.
fn bench_conversion_overhead(c: &mut Criterion) {
    let data = gen_mixed_realistic();
    let rows: u64 = 24;
    let cols: u64 = 80;

    let mut group = c.benchmark_group("conversion_overhead");
    group.throughput(Throughput::Elements(rows * cols));

    // Pre-parse data so the screen is populated
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(&data);

    group.bench_function("vt_to_rich_grid_80x24", |b| {
        b.iter(|| vt_to_rich_grid(parser.screen()));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_80x24,
    bench_pipeline_120x40,
    bench_pipeline_incremental,
    bench_conversion_overhead,
);
criterion_main!(benches);
