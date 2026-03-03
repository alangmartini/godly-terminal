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
}
