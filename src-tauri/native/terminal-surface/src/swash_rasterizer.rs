use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::FontRef;

use crate::glyph_rasterizer::{GlyphRasterizer, MeasuredFontMetrics, RasterizedGlyph};

/// Swash-based cross-platform glyph rasterizer.
///
/// Holds font data in memory and provides rasterization via the swash crate's
/// CPU-side glyph rendering pipeline. This avoids any GPU dependency and
/// produces alpha masks suitable for compositing into a pixel buffer.
pub struct SwashRasterizer {
    font_data: Vec<u8>,
    font_index: u32,
    scale_context: ScaleContext,
}

impl SwashRasterizer {
    pub fn new() -> Self {
        Self {
            font_data: Vec::new(),
            font_index: 0,
            scale_context: ScaleContext::new(),
        }
    }

    fn font_ref(&self) -> Option<FontRef<'_>> {
        if self.font_data.is_empty() {
            return None;
        }
        FontRef::from_index(&self.font_data, self.font_index as usize)
    }
}

impl GlyphRasterizer for SwashRasterizer {
    fn load_font(&mut self, data: &[u8], index: u32) -> bool {
        if FontRef::from_index(data, index as usize).is_none() {
            return false;
        }
        self.font_data = data.to_vec();
        self.font_index = index;
        true
    }

    fn rasterize(
        &mut self,
        ch: char,
        font_size_px: f32,
        bold: bool,
        _italic: bool,
    ) -> Option<RasterizedGlyph> {
        if self.font_data.is_empty() {
            return None;
        }

        let glyph_id = {
            let font = FontRef::from_index(&self.font_data, self.font_index as usize)?;
            let id = font.charmap().map(ch);
            if id == 0 {
                return None;
            }
            id
        };

        let (image_data, placement) = {
            let font = FontRef::from_index(&self.font_data, self.font_index as usize)?;
            let mut scaler = self
                .scale_context
                .builder(font)
                .size(font_size_px)
                .hint(true)
                .build();

            let mut render = Render::new(&[
                Source::ColorOutline(0),
                Source::ColorBitmap(StrikeWith::BestFit),
                Source::Outline,
            ]);
            render.format(Format::Alpha);

            if bold {
                render.embolden(0.02 * font_size_px);
            }

            let image = render.render(&mut scaler, glyph_id)?;
            (image.data, image.placement)
        };

        if placement.width == 0 || placement.height == 0 {
            return None;
        }

        let advance = {
            let font = FontRef::from_index(&self.font_data, self.font_index as usize)?;
            font.glyph_metrics(&[]).scale(font_size_px).advance_width(glyph_id)
        };

        Some(RasterizedGlyph {
            alpha: image_data,
            width: placement.width,
            height: placement.height,
            bearing_x: placement.left,
            bearing_y: placement.top,
            advance,
        })
    }

    fn measure(&mut self, font_size_px: f32) -> MeasuredFontMetrics {
        let font = match self.font_ref() {
            Some(f) => f,
            None => {
                return MeasuredFontMetrics {
                    ascent: font_size_px * 0.8,
                    descent: font_size_px * 0.2,
                    leading: 0.0,
                    average_advance: font_size_px * 0.6,
                    is_monospace: true,
                };
            }
        };

        let metrics = font.metrics(&[]).scale(font_size_px);
        let glyph_metrics = font.glyph_metrics(&[]).scale(font_size_px);

        let zero_glyph = font.charmap().map('0');
        let average_advance = if zero_glyph != 0 {
            glyph_metrics.advance_width(zero_glyph)
        } else {
            font_size_px * 0.6
        };

        // Heuristic monospace check: compare advance widths of 'M' and 'i'.
        // In a monospace font these should be identical.
        let is_monospace = {
            let m_glyph = font.charmap().map('M');
            let i_glyph = font.charmap().map('i');
            if m_glyph != 0 && i_glyph != 0 {
                let m_adv = glyph_metrics.advance_width(m_glyph);
                let i_adv = glyph_metrics.advance_width(i_glyph);
                (m_adv - i_adv).abs() < 0.01
            } else {
                true // assume monospace if we can't check
            }
        };

        MeasuredFontMetrics {
            ascent: metrics.ascent,
            descent: metrics.descent.abs(),
            leading: metrics.leading,
            average_advance,
            is_monospace,
        }
    }

    fn has_glyph(&self, ch: char) -> bool {
        match self.font_ref() {
            Some(font) => font.charmap().map(ch) != 0,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FONT: &[u8] =
        include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf");

    #[test]
    fn load_valid_font() {
        let mut rast = SwashRasterizer::new();
        assert!(rast.load_font(TEST_FONT, 0));
    }

    #[test]
    fn load_invalid_font_fails() {
        let mut rast = SwashRasterizer::new();
        assert!(!rast.load_font(b"not a font", 0));
    }

    #[test]
    fn rasterize_a_returns_nonempty_alpha() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        let glyph = rast.rasterize('A', 14.0, false, false).unwrap();
        assert!(glyph.width > 0);
        assert!(glyph.height > 0);
        assert!(!glyph.alpha.is_empty());
        assert_eq!(glyph.alpha.len(), (glyph.width * glyph.height) as usize);
    }

    #[test]
    fn rasterize_bold_changes_output() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        let normal = rast.rasterize('A', 14.0, false, false).unwrap();
        let bold = rast.rasterize('A', 14.0, true, false).unwrap();
        // Bold should produce a glyph (possibly wider or with more coverage)
        assert!(bold.width > 0);
        assert!(bold.height > 0);
        // At least some pixel values should differ
        let normal_sum: u64 = normal.alpha.iter().map(|&b| b as u64).sum();
        let bold_sum: u64 = bold.alpha.iter().map(|&b| b as u64).sum();
        assert_ne!(normal_sum, bold_sum, "bold should have different alpha coverage");
    }

    #[test]
    fn rasterize_unknown_char_returns_none() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        // Private use area character unlikely to exist in Geist Mono
        let result = rast.rasterize('\u{F0000}', 14.0, false, false);
        assert!(result.is_none());
    }

    #[test]
    fn measure_returns_sensible_values() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        let m = rast.measure(14.0);
        assert!(m.ascent > 0.0, "ascent should be positive");
        assert!(m.descent > 0.0, "descent should be positive (abs)");
        assert!(m.average_advance > 0.0, "advance should be positive");
        assert!(m.is_monospace, "Geist Mono should be detected as monospace");
    }

    #[test]
    fn has_glyph_ascii() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        assert!(rast.has_glyph('A'));
        assert!(rast.has_glyph('z'));
        assert!(rast.has_glyph('0'));
    }

    #[test]
    fn has_glyph_private_use_false() {
        let mut rast = SwashRasterizer::new();
        rast.load_font(TEST_FONT, 0);
        assert!(!rast.has_glyph('\u{F0000}'));
    }
}
