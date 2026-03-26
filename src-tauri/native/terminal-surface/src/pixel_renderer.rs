use std::sync::LazyLock;
use std::time::Instant;

use godly_protocol::types::{CursorShape, RichGridData};
use iced::Color;

use crate::colors::{brighten_color, dim_color, parse_color};
use crate::font_metrics::FontMetrics;
use crate::glyph_cache::{CachedGlyph, GlyphCache, GlyphKey};
use crate::glyph_rasterizer::GlyphRasterizer;
use crate::render_stats::RenderStats;
use crate::surface::GridPos;

// ---------------------------------------------------------------------------
// sRGB <-> linear colour-space lookup tables
// ---------------------------------------------------------------------------

/// Number of entries in the linear-to-sRGB reverse LUT.
/// 4096 gives ~0.4 LSB max quantisation error which is imperceptible.
const LIN_TO_SRGB_LUT_SIZE: usize = 4096;

/// sRGB (0-255) -> linear (0.0-1.0).
fn srgb_to_linear(s: u8) -> f32 {
    let s = s as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// linear (0.0-1.0) -> sRGB (0-255).
fn linear_to_srgb(l: f32) -> u8 {
    let s = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// Forward LUT: index by sRGB byte value, yields linear float.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    let mut table = [0.0f32; 256];
    for i in 0..256 {
        table[i] = srgb_to_linear(i as u8);
    }
    table
});

/// Reverse LUT: index by quantised linear value (0..4095), yields sRGB byte.
static LINEAR_TO_SRGB: LazyLock<[u8; LIN_TO_SRGB_LUT_SIZE]> = LazyLock::new(|| {
    let mut table = [0u8; LIN_TO_SRGB_LUT_SIZE];
    for i in 0..LIN_TO_SRGB_LUT_SIZE {
        let linear = i as f32 / (LIN_TO_SRGB_LUT_SIZE - 1) as f32;
        table[i] = linear_to_srgb(linear);
    }
    table
});

/// Convert a linear-space value (0.0..1.0) to an sRGB byte via the reverse LUT.
#[inline(always)]
fn linear_to_srgb_lut(l: f32) -> u8 {
    let clamped = l.clamp(0.0, 1.0);
    let idx = (clamped * (LIN_TO_SRGB_LUT_SIZE - 1) as f32 + 0.5) as usize;
    LINEAR_TO_SRGB[idx.min(LIN_TO_SRGB_LUT_SIZE - 1)]
}

/// CPU-side terminal grid compositor.
///
/// Renders a `RichGridData` snapshot into an RGBA pixel buffer suitable for
/// display via `iced::widget::image::Handle::from_rgba()`. The renderer
/// caches rasterized glyphs in a `GlyphCache` and composites them with
/// per-cell foreground/background colors, selection highlight, and cursor.
///
/// Character positions use fractional cell metrics (matching the canvas
/// renderer and grid dimension calculation) to avoid spacing mismatches.
pub struct PixelRenderer {
    buffer: Vec<u8>,
    width: u32,
    height: u32,
    last_stats: RenderStats,
}

impl PixelRenderer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            width: 0,
            height: 0,
            last_stats: RenderStats::default(),
        }
    }

    pub fn last_stats(&self) -> &RenderStats {
        &self.last_stats
    }

    /// Render a terminal grid into an RGBA pixel buffer.
    ///
    /// Returns `(pixels, width, height)`. The pixel buffer is RGBA with 4 bytes
    /// per pixel, row-major, and fully opaque.
    pub fn render(
        &mut self,
        grid: &RichGridData,
        metrics: &FontMetrics,
        cache: &mut GlyphCache,
        rasterizer: &mut dyn GlyphRasterizer,
        default_fg: Color,
        default_bg: Color,
        selection: Option<(GridPos, GridPos)>,
    ) -> (&[u8], u32, u32) {
        let t_start = Instant::now();

        let cols = grid.dimensions.cols as u32;
        let rows = grid.dimensions.rows as u32;
        let cell_w = metrics.cell_width;
        let cell_h = metrics.cell_height;

        if cols == 0 || rows == 0 || cell_w <= 0.0 || cell_h <= 0.0 {
            self.buffer.clear();
            self.width = 0;
            self.height = 0;
            self.last_stats = RenderStats::default();
            return (&self.buffer, 0, 0);
        }

        let w = (cols as f32 * cell_w).ceil() as u32;
        let h = (rows as f32 * cell_h).ceil() as u32;
        let total = (w * h * 4) as usize;

        if self.width != w || self.height != h {
            self.buffer.resize(total, 0);
            self.width = w;
            self.height = h;
        }

        let bg_r = (default_bg.r * 255.0) as u8;
        let bg_g = (default_bg.g * 255.0) as u8;
        let bg_b = (default_bg.b * 255.0) as u8;
        let t_bg = Instant::now();
        fill_solid(&mut self.buffer, bg_r, bg_g, bg_b, 255);
        let bg_fill = t_bg.elapsed();

        let t_glyph = Instant::now();
        let mut cells_rendered: u32 = 0;
        let mut rows_rendered: u32 = 0;
        for (row_idx, row) in grid.rows.iter().enumerate() {
            let cell_y = (row_idx as f32 * cell_h).round() as i32;
            let next_row_y = ((row_idx + 1) as f32 * cell_h).round() as i32;
            let row_h = (next_row_y - cell_y) as u32;
            rows_rendered += 1;

            for (col_idx, cell) in row.cells.iter().enumerate() {
                if cell.wide_continuation {
                    continue;
                }
                if !cell.content.is_empty() {
                    cells_rendered += 1;
                }

                let char_cols: u32 = if cell.wide { 2 } else { 1 };
                let cell_x = (col_idx as f32 * cell_w).round() as i32;
                let next_col_x = ((col_idx as u32 + char_cols) as f32 * cell_w).round() as i32;
                let col_w = (next_col_x - cell_x) as u32;

                let (mut fg, bg) = if cell.inverse {
                    (
                        parse_color(&cell.bg, default_bg),
                        parse_color(&cell.fg, default_fg),
                    )
                } else {
                    (
                        parse_color(&cell.fg, default_fg),
                        parse_color(&cell.bg, default_bg),
                    )
                };

                if cell.dim {
                    fg = dim_color(fg);
                }
                if cell.bold {
                    fg = brighten_color(fg);
                }

                let fg_r = (fg.r * 255.0) as u8;
                let fg_g = (fg.g * 255.0) as u8;
                let fg_b = (fg.b * 255.0) as u8;

                let cbg_r = (bg.r * 255.0) as u8;
                let cbg_g = (bg.g * 255.0) as u8;
                let cbg_b = (bg.b * 255.0) as u8;
                if cbg_r != bg_r || cbg_g != bg_g || cbg_b != bg_b {
                    fill_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cell_x,
                        cell_y,
                        col_w,
                        row_h,
                        cbg_r,
                        cbg_g,
                        cbg_b,
                        255,
                    );
                }

                if !cell.content.is_empty() && cell.content != " " {
                    let ch = match cell.content.chars().next() {
                        Some(c) => c,
                        None => continue,
                    };

                    let key = GlyphKey::new(ch, metrics.font_size, cell.bold, cell.italic);

                    if cache.get(&key).is_none() {
                        if let Some(rg) =
                            rasterizer.rasterize(ch, metrics.font_size, cell.bold, cell.italic)
                        {
                            cache.insert(
                                key,
                                CachedGlyph {
                                    alpha: rg.alpha,
                                    width: rg.width,
                                    height: rg.height,
                                    bearing_x: rg.bearing_x,
                                    bearing_y: rg.bearing_y,
                                    advance: rg.advance,
                                },
                            );
                        } else {
                            continue;
                        }
                    }
                    let glyph_ref = cache.get(&key).unwrap();

                    let glyph_x = cell_x + glyph_ref.bearing_x;
                    let glyph_y = cell_y + (metrics.baseline_offset as i32 - glyph_ref.bearing_y);

                    blit_alpha(
                        &mut self.buffer,
                        w,
                        h,
                        glyph_ref,
                        glyph_x,
                        glyph_y,
                        fg_r,
                        fg_g,
                        fg_b,
                    );
                }

                if cell.underline {
                    let uy = next_row_y - 2;
                    fill_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cell_x,
                        uy,
                        col_w,
                        1,
                        fg_r,
                        fg_g,
                        fg_b,
                        255,
                    );
                }
            }
        }
        let glyph_phase = t_glyph.elapsed();

        let t_sel = Instant::now();
        if let Some((start, end)) = selection {
            let sel_r: u8 = 51;
            let sel_g: u8 = 102;
            let sel_b: u8 = 204;
            let sel_a: u8 = 77;

            for row in start.row..=end.row {
                if row >= grid.rows.len() {
                    break;
                }
                let y = (row as f32 * cell_h).round() as i32;
                let next_y = ((row + 1) as f32 * cell_h).round() as i32;
                let rh = (next_y - y) as u32;
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row {
                    end.col
                } else {
                    grid.rows[row].cells.len().saturating_sub(1)
                };
                let x = (col_start as f32 * cell_w).round() as i32;
                let x_end = ((col_end + 1) as f32 * cell_w).round() as i32;
                let rw = (x_end - x) as u32;
                blend_rect(
                    &mut self.buffer,
                    w,
                    h,
                    x,
                    y,
                    rw,
                    rh,
                    sel_r,
                    sel_g,
                    sel_b,
                    sel_a,
                );
            }
        }
        let selection_phase = t_sel.elapsed();

        let t_cur = Instant::now();
        if !grid.cursor_hidden {
            let cursor_x = (grid.cursor.col as f32 * cell_w).round() as i32;
            let cursor_y = (grid.cursor.row as f32 * cell_h).round() as i32;
            let next_cx = ((grid.cursor.col as f32 + 1.0) * cell_w).round() as i32;
            let next_cy = ((grid.cursor.row as f32 + 1.0) * cell_h).round() as i32;
            let cw = (next_cx - cursor_x) as u32;
            let ch = (next_cy - cursor_y) as u32;
            let cur_r: u8 = 255;
            let cur_g: u8 = 255;
            let cur_b: u8 = 255;
            let cur_a: u8 = 204;

            match grid.cursor.cursor_style {
                CursorShape::BlinkBlock | CursorShape::SteadyBlock => {
                    blend_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cursor_x,
                        cursor_y,
                        cw,
                        ch,
                        cur_r,
                        cur_g,
                        cur_b,
                        cur_a,
                    );
                }
                CursorShape::BlinkUnderline | CursorShape::SteadyUnderline => {
                    let underline_h = 2u32;
                    let uy = next_cy - underline_h as i32;
                    blend_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cursor_x,
                        uy,
                        cw,
                        underline_h,
                        cur_r,
                        cur_g,
                        cur_b,
                        cur_a,
                    );
                }
                CursorShape::BlinkBar | CursorShape::SteadyBar => {
                    let bar_w = 2u32;
                    blend_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cursor_x,
                        cursor_y,
                        bar_w,
                        ch,
                        cur_r,
                        cur_g,
                        cur_b,
                        cur_a,
                    );
                }
            }
        }
        let cursor_phase = t_cur.elapsed();

        self.last_stats = RenderStats {
            bg_fill,
            glyph_phase,
            cursor_phase,
            selection_phase,
            total: t_start.elapsed(),
            cells_rendered,
            rows_rendered,
        };

        (&self.buffer, w, h)
    }
}

/// Fill the entire buffer with a solid color.
fn fill_solid(buf: &mut [u8], r: u8, g: u8, b: u8, a: u8) {
    let pixel = [r, g, b, a];
    for chunk in buf.chunks_exact_mut(4) {
        chunk.copy_from_slice(&pixel);
    }
}

/// Fill a rectangle with a solid color. Coordinates are i32 to support
/// fractional cell positioning that may round to negative values at edges.
fn fill_rect(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x_end = ((x as i64 + w as i64) as u32).min(buf_w);
    let y_end = ((y as i64 + h as i64) as u32).min(buf_h);
    for py in y0..y_end {
        for px in x0..x_end {
            let idx = ((py * buf_w + px) * 4) as usize;
            if idx + 3 < buf.len() {
                buf[idx] = r;
                buf[idx + 1] = g;
                buf[idx + 2] = b;
                buf[idx + 3] = a;
            }
        }
    }
}

/// Blend a semi-transparent rectangle over existing pixels (gamma-correct).
///
/// Converts both source and destination from sRGB to linear light, performs
/// alpha compositing in linear space, then converts the result back to sRGB.
fn blend_rect(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let lut = &*SRGB_TO_LINEAR;
    let src_r = lut[r as usize];
    let src_g = lut[g as usize];
    let src_b = lut[b as usize];
    let alpha = a as f32 / 255.0;
    let inv_a = 1.0 - alpha;

    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x_end = ((x as i64 + w as i64) as u32).min(buf_w);
    let y_end = ((y as i64 + h as i64) as u32).min(buf_h);
    for py in y0..y_end {
        for px in x0..x_end {
            let idx = ((py * buf_w + px) * 4) as usize;
            if idx + 3 < buf.len() {
                let bg_r = lut[buf[idx] as usize];
                let bg_g = lut[buf[idx + 1] as usize];
                let bg_b = lut[buf[idx + 2] as usize];

                buf[idx] = linear_to_srgb_lut(src_r * alpha + bg_r * inv_a);
                buf[idx + 1] = linear_to_srgb_lut(src_g * alpha + bg_g * inv_a);
                buf[idx + 2] = linear_to_srgb_lut(src_b * alpha + bg_b * inv_a);
                buf[idx + 3] = 255;
            }
        }
    }
}

/// Blit a glyph's alpha mask onto the buffer with foreground color tinting
/// (gamma-correct).
///
/// Alpha compositing is performed in linear light so that thin strokes blend
/// correctly against any background colour.
fn blit_alpha(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    glyph: &CachedGlyph,
    glyph_x: i32,
    glyph_y: i32,
    fg_r: u8,
    fg_g: u8,
    fg_b: u8,
) {
    let lut = &*SRGB_TO_LINEAR;
    let fg_r_lin = lut[fg_r as usize];
    let fg_g_lin = lut[fg_g as usize];
    let fg_b_lin = lut[fg_b as usize];

    for gy in 0..glyph.height {
        let dest_y = glyph_y + gy as i32;
        if dest_y < 0 || dest_y >= buf_h as i32 {
            continue;
        }
        for gx in 0..glyph.width {
            let dest_x = glyph_x + gx as i32;
            if dest_x < 0 || dest_x >= buf_w as i32 {
                continue;
            }

            let alpha = glyph.alpha[(gy * glyph.width + gx) as usize];
            if alpha == 0 {
                continue;
            }

            let idx = ((dest_y as u32 * buf_w + dest_x as u32) * 4) as usize;
            if idx + 3 >= buf.len() {
                continue;
            }

            if alpha == 255 {
                // Fully opaque: no blending needed, just write fg colour.
                buf[idx] = fg_r;
                buf[idx + 1] = fg_g;
                buf[idx + 2] = fg_b;
                buf[idx + 3] = 255;
                continue;
            }

            let a = alpha as f32 / 255.0;
            let inv_a = 1.0 - a;

            let bg_r_lin = lut[buf[idx] as usize];
            let bg_g_lin = lut[buf[idx + 1] as usize];
            let bg_b_lin = lut[buf[idx + 2] as usize];

            buf[idx] = linear_to_srgb_lut(fg_r_lin * a + bg_r_lin * inv_a);
            buf[idx + 1] = linear_to_srgb_lut(fg_g_lin * a + bg_g_lin * inv_a);
            buf[idx + 2] = linear_to_srgb_lut(fg_b_lin * a + bg_b_lin * inv_a);
            buf[idx + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use godly_protocol::types::{
        CursorState, GridDimensions, RichGridCell, RichGridData, RichGridRow,
    };

    fn make_cell(content: &str) -> RichGridCell {
        RichGridCell {
            content: content.to_string(),
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

    fn make_grid(rows: u16, cols: u16, cells: Vec<Vec<RichGridCell>>) -> RichGridData {
        RichGridData {
            rows: cells
                .into_iter()
                .map(|c| RichGridRow {
                    cells: c,
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
            cursor_hidden: true,
            title: String::new(),
            scrollback_offset: 0,
            total_scrollback: 0,
        }
    }

    /// Stub rasterizer for unit tests (returns a known alpha pattern).
    struct StubRasterizer;

    impl GlyphRasterizer for StubRasterizer {
        fn rasterize(
            &mut self,
            _ch: char,
            _font_size_px: f32,
            _bold: bool,
            _italic: bool,
        ) -> Option<crate::glyph_rasterizer::RasterizedGlyph> {
            Some(crate::glyph_rasterizer::RasterizedGlyph {
                alpha: vec![128; 4], // 2x2 alpha mask
                width: 2,
                height: 2,
                bearing_x: 0,
                bearing_y: 2,
                advance: 8.0,
            })
        }

        fn measure(&mut self, font_size_px: f32) -> crate::glyph_rasterizer::MeasuredFontMetrics {
            crate::glyph_rasterizer::MeasuredFontMetrics {
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

    #[test]
    fn render_1_cell_grid_correct_dimensions() {
        let grid = make_grid(1, 1, vec![vec![make_cell("A")]]);
        let metrics = FontMetrics::from_font_size(14.0);
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        let (buf, w, h) = renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        // Buffer size derived from fractional metrics: ceil(1 * cell_w) x ceil(1 * cell_h)
        let expected_w = metrics.cell_width.ceil() as u32;
        let expected_h = metrics.cell_height.ceil() as u32;
        assert_eq!(w, expected_w);
        assert_eq!(h, expected_h);
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn render_empty_grid_returns_zero() {
        let grid = make_grid(0, 0, vec![]);
        let metrics = FontMetrics::from_font_size(14.0);
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        let (buf, w, h) = renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        assert_eq!(w, 0);
        assert_eq!(h, 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn fill_rect_correct_pixels() {
        let w = 4u32;
        let h = 4u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];

        fill_rect(&mut buf, w, h, 1, 1, 2, 2, 255, 0, 0, 255);

        let idx = ((1 * w + 1) * 4) as usize;
        assert_eq!(buf[idx], 255);
        assert_eq!(buf[idx + 1], 0);
        assert_eq!(buf[idx + 2], 0);
        assert_eq!(buf[idx + 3], 255);

        assert_eq!(buf[0], 0);
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
    }

    #[test]
    fn blit_known_alpha_composites_correctly() {
        let w = 4u32;
        let h = 4u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];

        fill_solid(&mut buf, 255, 255, 255, 255);

        let glyph = CachedGlyph {
            alpha: vec![128; 4], // 2x2, 50% alpha
            width: 2,
            height: 2,
            bearing_x: 0,
            bearing_y: 0,
            advance: 8.0,
        };

        blit_alpha(&mut buf, w, h, &glyph, 1, 1, 255, 0, 0);

        // Gamma-correct blending: red (255,0,0) at ~50% alpha over white.
        // In linear space: G/B = 0.0 * 0.502 + 1.0 * 0.498 = 0.498
        // Back to sRGB: ~188  (brighter than naive 127 because sRGB is non-linear)
        let idx = ((1 * w + 1) * 4) as usize;
        assert_eq!(buf[idx], 255); // R stays 255
        assert!(
            (buf[idx + 1] as i32 - 188).abs() <= 2,
            "G expected ~188, got {}",
            buf[idx + 1]
        );
        assert!(
            (buf[idx + 2] as i32 - 188).abs() <= 2,
            "B expected ~188, got {}",
            buf[idx + 2]
        );
        assert_eq!(buf[idx + 3], 255); // A

        // Untouched pixel remains white
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
    }

    #[test]
    fn render_populates_glyph_cache() {
        let grid = make_grid(1, 2, vec![vec![make_cell("A"), make_cell("B")]]);
        let metrics = FontMetrics::from_font_size(14.0);
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        assert!(cache.is_empty());

        renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn fractional_positioning_matches_grid_extent() {
        // Verify buffer width = ceil(cols * cell_w), not cols * ceil(cell_w)
        let grid = make_grid(
            2,
            10,
            vec![vec![make_cell(" "); 10], vec![make_cell(" "); 10]],
        );
        let metrics = FontMetrics::from_font_size(14.0); // cell_w=8.4
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        let (_, w, _) = renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        // ceil(10 * 8.4) = ceil(84.0) = 84, NOT 10 * ceil(8.4) = 10 * 9 = 90
        let expected = (10.0_f32 * metrics.cell_width).ceil() as u32;
        assert_eq!(w, expected);
    }

    #[test]
    fn render_populates_last_stats() {
        let grid = make_grid(
            2,
            3,
            vec![
                vec![make_cell("A"), make_cell("B"), make_cell("C")],
                vec![make_cell("D"), make_cell(" "), make_cell("F")],
            ],
        );
        let metrics = FontMetrics::from_font_size(14.0);
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        let stats = renderer.last_stats();
        assert!(stats.rows_rendered > 0);
        assert!(stats.cells_rendered > 0);
        assert!(stats.total > std::time::Duration::ZERO);
    }

    #[test]
    fn empty_grid_stats() {
        let grid = make_grid(0, 0, vec![]);
        let metrics = FontMetrics::from_font_size(14.0);
        let mut cache = GlyphCache::new();
        let mut rast = StubRasterizer;
        let mut renderer = PixelRenderer::new();

        renderer.render(
            &grid,
            &metrics,
            &mut cache,
            &mut rast,
            Color::WHITE,
            Color::BLACK,
            None,
        );

        let stats = renderer.last_stats();
        assert_eq!(stats.rows_rendered, 0);
        assert_eq!(stats.cells_rendered, 0);
        assert_eq!(stats.total, std::time::Duration::ZERO);
    }
}
