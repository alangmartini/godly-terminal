use godly_protocol::types::{CursorShape, RichGridData};
use iced::Color;

use crate::colors::{brighten_color, dim_color, parse_color};
use crate::font_metrics::FontMetrics;
use crate::glyph_cache::{CachedGlyph, GlyphCache, GlyphKey};
use crate::glyph_rasterizer::GlyphRasterizer;
use crate::surface::GridPos;

/// CPU-side terminal grid compositor.
///
/// Renders a `RichGridData` snapshot into an RGBA pixel buffer suitable for
/// display via `iced::widget::image::Handle::from_rgba()`. The renderer
/// caches rasterized glyphs in a `GlyphCache` and composites them with
/// per-cell foreground/background colors, selection highlight, and cursor.
pub struct PixelRenderer {
    buffer: Vec<u8>,
    width: u32,
    height: u32,
}

impl PixelRenderer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            width: 0,
            height: 0,
        }
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
        let cols = grid.dimensions.cols as u32;
        let rows = grid.dimensions.rows as u32;
        let cell_w = metrics.cell_width.ceil() as u32;
        let cell_h = metrics.cell_height.ceil() as u32;

        if cols == 0 || rows == 0 || cell_w == 0 || cell_h == 0 {
            self.buffer.clear();
            self.width = 0;
            self.height = 0;
            return (&self.buffer, 0, 0);
        }

        let w = cols * cell_w;
        let h = rows * cell_h;
        let total = (w * h * 4) as usize;

        if self.width != w || self.height != h {
            self.buffer.resize(total, 0);
            self.width = w;
            self.height = h;
        }

        let bg_r = (default_bg.r * 255.0) as u8;
        let bg_g = (default_bg.g * 255.0) as u8;
        let bg_b = (default_bg.b * 255.0) as u8;
        fill_solid(&mut self.buffer, w, h, bg_r, bg_g, bg_b, 255);

        for (row_idx, row) in grid.rows.iter().enumerate() {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                if cell.wide_continuation {
                    continue;
                }

                let cell_x = col_idx as u32 * cell_w;
                let cell_y = row_idx as u32 * cell_h;
                let char_width: u32 = if cell.wide { 2 } else { 1 };

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
                        cell_w * char_width,
                        cell_h,
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
                        if let Some(rg) = rasterizer.rasterize(
                            ch,
                            metrics.font_size,
                            cell.bold,
                            cell.italic,
                        ) {
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

                    let glyph_x = cell_x as i32 + glyph_ref.bearing_x;
                    let glyph_y =
                        cell_y as i32 + (metrics.baseline_offset as i32 - glyph_ref.bearing_y);

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
                    let uy = cell_y + cell_h - 2;
                    fill_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cell_x,
                        uy,
                        cell_w * char_width,
                        1,
                        fg_r,
                        fg_g,
                        fg_b,
                        255,
                    );
                }
            }
        }

        if let Some((start, end)) = selection {
            let sel_r: u8 = 51;
            let sel_g: u8 = 102;
            let sel_b: u8 = 204;
            let sel_a: u8 = 77;

            for row in start.row..=end.row {
                if row >= grid.rows.len() {
                    break;
                }
                let y = row as u32 * cell_h;
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row {
                    end.col
                } else {
                    grid.rows[row].cells.len().saturating_sub(1)
                };
                let x = col_start as u32 * cell_w;
                let width = ((col_end - col_start + 1) as u32) * cell_w;
                blend_rect(
                    &mut self.buffer,
                    w,
                    h,
                    x,
                    y,
                    width,
                    cell_h,
                    sel_r,
                    sel_g,
                    sel_b,
                    sel_a,
                );
            }
        }

        if !grid.cursor_hidden {
            let cursor_x = grid.cursor.col as u32 * cell_w;
            let cursor_y = grid.cursor.row as u32 * cell_h;
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
                        cell_w,
                        cell_h,
                        cur_r,
                        cur_g,
                        cur_b,
                        cur_a,
                    );
                }
                CursorShape::BlinkUnderline | CursorShape::SteadyUnderline => {
                    let underline_h = 2u32;
                    let uy = cursor_y + cell_h.saturating_sub(underline_h);
                    blend_rect(
                        &mut self.buffer,
                        w,
                        h,
                        cursor_x,
                        uy,
                        cell_w,
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
                        cell_h,
                        cur_r,
                        cur_g,
                        cur_b,
                        cur_a,
                    );
                }
            }
        }

        (&self.buffer, w, h)
    }
}

/// Fill the entire buffer with a solid color.
fn fill_solid(buf: &mut [u8], _w: u32, _h: u32, r: u8, g: u8, b: u8, a: u8) {
    let pixel = [r, g, b, a];
    for chunk in buf.chunks_exact_mut(4) {
        chunk.copy_from_slice(&pixel);
    }
}

/// Fill a rectangle with a solid color.
fn fill_rect(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let x_end = (x + w).min(buf_w);
    let y_end = (y + h).min(buf_h);
    for py in y..y_end {
        for px in x..x_end {
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

/// Blend a semi-transparent rectangle over existing pixels.
fn blend_rect(
    buf: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let x_end = (x + w).min(buf_w);
    let y_end = (y + h).min(buf_h);
    let src_a = a as u16;
    let inv_a = 255 - src_a;
    for py in y..y_end {
        for px in x..x_end {
            let idx = ((py * buf_w + px) * 4) as usize;
            if idx + 3 < buf.len() {
                buf[idx] = ((r as u16 * src_a + buf[idx] as u16 * inv_a) / 255) as u8;
                buf[idx + 1] = ((g as u16 * src_a + buf[idx + 1] as u16 * inv_a) / 255) as u8;
                buf[idx + 2] = ((b as u16 * src_a + buf[idx + 2] as u16 * inv_a) / 255) as u8;
                buf[idx + 3] = 255;
            }
        }
    }
}

/// Blit a glyph's alpha mask onto the buffer with foreground color tinting.
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

            let src_a = alpha as u16;
            let inv_a = 255 - src_a;
            buf[idx] = ((fg_r as u16 * src_a + buf[idx] as u16 * inv_a) / 255) as u8;
            buf[idx + 1] = ((fg_g as u16 * src_a + buf[idx + 1] as u16 * inv_a) / 255) as u8;
            buf[idx + 2] = ((fg_b as u16 * src_a + buf[idx + 2] as u16 * inv_a) / 255) as u8;
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

        let cell_w = metrics.cell_width.ceil() as u32;
        let cell_h = metrics.cell_height.ceil() as u32;
        assert_eq!(w, cell_w);
        assert_eq!(h, cell_h);
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

        // Check pixel at (1,1) is red
        let idx = ((1 * w + 1) * 4) as usize;
        assert_eq!(buf[idx], 255);
        assert_eq!(buf[idx + 1], 0);
        assert_eq!(buf[idx + 2], 0);
        assert_eq!(buf[idx + 3], 255);

        // Check pixel at (0,0) is still black
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

        // Fill background white
        fill_solid(&mut buf, w, h, 255, 255, 255, 255);

        let glyph = CachedGlyph {
            alpha: vec![128; 4], // 2x2, 50% alpha
            width: 2,
            height: 2,
            bearing_x: 0,
            bearing_y: 0,
            advance: 8.0,
        };

        // Blit red glyph at (1,1)
        blit_alpha(&mut buf, w, h, &glyph, 1, 1, 255, 0, 0);

        // Pixel at (1,1): blend red (128/255 ~ 50%) over white
        let idx = ((1 * w + 1) * 4) as usize;
        // Expected R: (255 * 128 + 255 * 127) / 255 = 255
        // Expected G: (0 * 128 + 255 * 127) / 255 ≈ 127
        // Expected B: (0 * 128 + 255 * 127) / 255 ≈ 127
        assert_eq!(buf[idx], 255); // R
        assert!((buf[idx + 1] as i32 - 127).abs() <= 1); // G
        assert!((buf[idx + 2] as i32 - 127).abs() <= 1); // B
        assert_eq!(buf[idx + 3], 255); // A

        // Pixel at (0,0) should still be pure white
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

        // 'A' and 'B' should both be cached
        assert_eq!(cache.len(), 2);
    }
}
