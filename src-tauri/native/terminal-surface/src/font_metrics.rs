/// Font metrics for monospace terminal rendering.
///
/// Provides cell dimensions derived from font size using heuristic ratios.
/// These are reasonable defaults for monospace fonts and can be replaced
/// with measured values once actual font shaping is available.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// Width of a single cell in pixels.
    pub cell_width: f32,
    /// Height of a single cell in pixels.
    pub cell_height: f32,
    /// Font size in pixels.
    pub font_size: f32,
    /// Vertical offset from cell top to text baseline in pixels.
    pub baseline_offset: f32,
}

impl FontMetrics {
    /// Width-to-font-size ratio for monospace fonts.
    const WIDTH_RATIO: f32 = 0.6;
    /// Height-to-font-size ratio for monospace fonts.
    const HEIGHT_RATIO: f32 = 1.3;
    /// Baseline position as fraction of cell height.
    const BASELINE_FRACTION: f32 = 0.75;

    /// Create font metrics from a font size using heuristic ratios.
    ///
    /// - `cell_width = font_size * 0.6`
    /// - `cell_height = font_size * 1.3`
    /// - `baseline_offset = cell_height * 0.75`
    pub fn from_font_size(font_size: f32) -> Self {
        let cell_width = font_size * Self::WIDTH_RATIO;
        let cell_height = font_size * Self::HEIGHT_RATIO;
        let baseline_offset = cell_height * Self::BASELINE_FRACTION;
        Self {
            cell_width,
            cell_height,
            font_size,
            baseline_offset,
        }
    }

    /// Create font metrics by measuring the actual font file via swash.
    ///
    /// Reads ascent, descent, and leading from the font's OS/2 and hhea tables,
    /// and measures the advance width of the '0' glyph for the cell width.
    /// Falls back to [`from_font_size`](Self::from_font_size) if the font
    /// data cannot be parsed.
    pub fn from_font_bytes(font_size: f32, font_data: &[u8]) -> Self {
        let font = match swash::FontRef::from_index(font_data, 0) {
            Some(f) => f,
            None => return Self::from_font_size(font_size),
        };

        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;
        if upem == 0.0 {
            return Self::from_font_size(font_size);
        }

        let scale = font_size / upem;
        let ascent = metrics.ascent * scale;
        let descent = metrics.descent.abs() * scale;
        let leading = metrics.leading * scale;

        let cell_height = ascent + descent + leading;
        let baseline_offset = ascent;

        // Measure advance width of '0' for cell width
        let charmap = font.charmap();
        let glyph_id = charmap.map('0');
        let glyph_metrics = font.glyph_metrics(&[]).scale(font_size);
        let cell_width = glyph_metrics.advance_width(glyph_id);

        // Sanity check: if measurements look unreasonable, fall back
        if cell_width <= 0.0 || cell_height <= 0.0 || baseline_offset <= 0.0 {
            return Self::from_font_size(font_size);
        }

        Self {
            cell_width,
            cell_height,
            font_size,
            baseline_offset,
        }
    }
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self::from_font_size(14.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_font_size_14() {
        let m = FontMetrics::from_font_size(14.0);
        assert!((m.cell_width - 8.4).abs() < 0.01);
        assert!((m.cell_height - 18.2).abs() < 0.01);
        assert!((m.font_size - 14.0).abs() < 0.01);
        assert!((m.baseline_offset - 13.65).abs() < 0.01);
    }

    #[test]
    fn from_font_size_16() {
        let m = FontMetrics::from_font_size(16.0);
        assert!((m.cell_width - 9.6).abs() < 0.01);
        assert!((m.cell_height - 20.8).abs() < 0.01);
        assert!((m.font_size - 16.0).abs() < 0.01);
        assert!((m.baseline_offset - 15.6).abs() < 0.01);
    }

    #[test]
    fn from_font_size_zero() {
        let m = FontMetrics::from_font_size(0.0);
        assert!((m.cell_width).abs() < 0.01);
        assert!((m.cell_height).abs() < 0.01);
        assert!((m.baseline_offset).abs() < 0.01);
    }

    #[test]
    fn default_uses_14() {
        let m = FontMetrics::default();
        let expected = FontMetrics::from_font_size(14.0);
        assert_eq!(m, expected);
    }

    #[test]
    fn width_height_ratio_relationship() {
        // For any font size, width should be narrower than height (monospace convention)
        for size in [8.0, 12.0, 14.0, 16.0, 20.0, 24.0] {
            let m = FontMetrics::from_font_size(size);
            assert!(
                m.cell_width < m.cell_height,
                "cell_width ({}) should be less than cell_height ({}) for font_size {}",
                m.cell_width,
                m.cell_height,
                size
            );
        }
    }

    #[test]
    fn baseline_within_cell() {
        // Baseline must be within cell bounds for readable text
        for size in [8.0, 12.0, 14.0, 16.0, 20.0, 24.0] {
            let m = FontMetrics::from_font_size(size);
            assert!(
                m.baseline_offset > 0.0 && m.baseline_offset < m.cell_height,
                "baseline_offset ({}) should be within (0, {}) for font_size {}",
                m.baseline_offset,
                m.cell_height,
                size
            );
        }
    }

    /// Geist Mono Regular font bytes for testing measured metrics.
    const GEIST_MONO: &[u8] = include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf");

    #[test]
    fn from_font_bytes_cell_width_positive_and_less_than_font_size() {
        for size in [12.0, 14.0, 16.0, 20.0] {
            let m = FontMetrics::from_font_bytes(size, GEIST_MONO);
            assert!(
                m.cell_width > 0.0 && m.cell_width < size,
                "cell_width ({}) should be > 0 and < font_size ({}) for monospace",
                m.cell_width,
                size,
            );
        }
    }

    #[test]
    fn from_font_bytes_cell_height_greater_than_width() {
        let m = FontMetrics::from_font_bytes(14.0, GEIST_MONO);
        assert!(
            m.cell_height > m.cell_width,
            "cell_height ({}) should be > cell_width ({}) for monospace",
            m.cell_height,
            m.cell_width,
        );
    }

    #[test]
    fn from_font_bytes_baseline_within_cell() {
        let m = FontMetrics::from_font_bytes(14.0, GEIST_MONO);
        assert!(
            m.baseline_offset > 0.0 && m.baseline_offset < m.cell_height,
            "baseline_offset ({}) should be within (0, {})",
            m.baseline_offset,
            m.cell_height,
        );
    }

    #[test]
    fn from_font_bytes_invalid_data_falls_back() {
        let m = FontMetrics::from_font_bytes(14.0, b"not a font");
        let fallback = FontMetrics::from_font_size(14.0);
        assert_eq!(m, fallback, "invalid font data should fall back to heuristic");
    }

    #[test]
    fn from_font_bytes_empty_data_falls_back() {
        let m = FontMetrics::from_font_bytes(14.0, &[]);
        let fallback = FontMetrics::from_font_size(14.0);
        assert_eq!(m, fallback, "empty font data should fall back to heuristic");
    }
}
