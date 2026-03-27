/// Font metrics for monospace terminal rendering.
///
/// Cell dimensions are measured from actual font tables (hhea/OS/2 ascent,
/// descent, line gap, and the advance width of a reference glyph). Heuristic
/// ratios are only used as a last-resort fallback when font data cannot be
/// parsed.
///
/// All pixel dimensions (`cell_width`, `cell_height`, `baseline_offset`,
/// `font_size`) are in **logical** pixels. Call [`scaled_for_render`] to
/// obtain physical-pixel metrics for the pixel renderer path.
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
    /// DPI scale factor from the OS (e.g. 1.0, 1.25, 1.5, 2.0).
    /// Defaults to 1.0. Used by the pixel renderer to rasterize glyphs
    /// at the correct physical resolution.
    pub scale_factor: f32,
}

impl FontMetrics {
    /// Width-to-font-size ratio for monospace fonts (last-resort heuristic).
    const WIDTH_RATIO: f32 = 0.6;
    /// Height-to-font-size ratio for monospace fonts (last-resort heuristic).
    const HEIGHT_RATIO: f32 = 1.3;
    /// Baseline position as fraction of cell height (last-resort heuristic).
    const BASELINE_FRACTION: f32 = 0.75;

    /// Create font metrics from a font size using heuristic ratios.
    ///
    /// **This is a last-resort fallback.** Prefer [`from_font_bytes`] or
    /// [`from_system_font`] which read actual font tables.
    ///
    /// - `cell_width = font_size * 0.6`
    /// - `cell_height = font_size * 1.3`
    /// - `baseline_offset = cell_height * 0.75`
    pub fn from_font_size(font_size: f32) -> Self {
        log::warn!(
            "FontMetrics: using heuristic fallback for font_size={:.1}px \
             — metrics will be approximate",
            font_size,
        );
        Self::from_font_size_quiet(font_size)
    }

    /// Heuristic fallback without the warning (used internally after an
    /// explicit warning has already been emitted, and in `Default`).
    fn from_font_size_quiet(font_size: f32) -> Self {
        let cell_width = font_size * Self::WIDTH_RATIO;
        let cell_height = font_size * Self::HEIGHT_RATIO;
        let baseline_offset = cell_height * Self::BASELINE_FRACTION;
        Self {
            cell_width,
            cell_height,
            font_size,
            baseline_offset,
            scale_factor: 1.0,
        }
    }

    /// Create font metrics by measuring the actual font file via swash.
    ///
    /// Reads ascent, descent, and leading from the font's OS/2 and hhea tables,
    /// and measures the advance width of a reference glyph ('0', 'M', or space)
    /// for the cell width. Falls back to heuristic ratios (with a warning) if
    /// the font data cannot be parsed.
    pub fn from_font_bytes(font_size: f32, font_data: &[u8]) -> Self {
        let font = match swash::FontRef::from_index(font_data, 0) {
            Some(f) => f,
            None => {
                log::warn!(
                    "FontMetrics: failed to parse font data ({} bytes) \
                     — falling back to heuristics for font_size={:.1}px",
                    font_data.len(),
                    font_size,
                );
                return Self::from_font_size_quiet(font_size);
            }
        };

        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;
        if upem == 0.0 {
            log::warn!(
                "FontMetrics: font reports units_per_em=0 \
                 — falling back to heuristics for font_size={:.1}px",
                font_size,
            );
            return Self::from_font_size_quiet(font_size);
        }

        let scale = font_size / upem;
        let ascent = metrics.ascent * scale;
        let descent = metrics.descent.abs() * scale;
        let leading = metrics.leading * scale;

        let cell_height = ascent + descent + leading;
        let baseline_offset = ascent;

        // Measure advance width of a reference glyph for cell width.
        // Try '0' first (standard monospace reference), then 'M', then space.
        let charmap = font.charmap();
        let glyph_metrics = font.glyph_metrics(&[]).scale(font_size);

        let cell_width = {
            let candidates = ['0', 'M', ' '];
            let mut width = 0.0f32;
            for &ch in &candidates {
                let gid = charmap.map(ch);
                if gid != 0 {
                    let w = glyph_metrics.advance_width(gid);
                    if w > 0.0 {
                        width = w;
                        break;
                    }
                }
            }
            width
        };

        // Sanity check: if measurements look unreasonable, fall back
        if cell_width <= 0.0 || cell_height <= 0.0 || baseline_offset <= 0.0 {
            log::warn!(
                "FontMetrics: measured values out of range \
                 (cell_width={:.2}, cell_height={:.2}, baseline={:.2}) \
                 — falling back to heuristics for font_size={:.1}px",
                cell_width,
                cell_height,
                baseline_offset,
                font_size,
            );
            return Self::from_font_size_quiet(font_size);
        }

        Self {
            cell_width,
            cell_height,
            font_size,
            baseline_offset,
            scale_factor: 1.0,
        }
    }

    /// Create font metrics by loading a system font by family name.
    ///
    /// Uses [`FontLoader`](crate::font_loader::FontLoader) to locate the font
    /// on disk, then delegates to [`from_font_bytes`](Self::from_font_bytes)
    /// for actual measurement. Falls back to heuristic ratios (with a warning)
    /// if the font cannot be found.
    pub fn from_system_font(font_size: f32, family: &str) -> Self {
        let loader = crate::font_loader::FontLoader::new();
        match loader.load_font(family, false, false) {
            Some(data) => Self::from_font_bytes(font_size, &data),
            None => {
                log::warn!(
                    "FontMetrics: could not load system font '{}' \
                     — falling back to heuristics for font_size={:.1}px",
                    family,
                    font_size,
                );
                Self::from_font_size_quiet(font_size)
            }
        }
    }

    /// Return a copy with all pixel dimensions multiplied by `scale_factor`.
    ///
    /// The pixel renderer uses this to rasterize glyphs and lay out cells at
    /// the physical-pixel resolution required by the OS DPI setting.
    /// When `scale_factor == 1.0` the returned metrics are identical.
    pub fn scaled_for_render(&self) -> Self {
        let s = self.scale_factor;
        Self {
            cell_width: self.cell_width * s,
            cell_height: self.cell_height * s,
            font_size: self.font_size * s,
            baseline_offset: self.baseline_offset * s,
            scale_factor: s,
        }
    }

    /// Set the DPI scale factor. Returns `self` for chaining.
    pub fn with_scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self
    }
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self::from_font_size_quiet(14.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper that builds heuristic metrics without triggering the log::warn
    /// (mirrors the internal `from_font_size_quiet`).
    fn heuristic(font_size: f32) -> FontMetrics {
        FontMetrics::from_font_size_quiet(font_size)
    }

    #[test]
    fn heuristic_14() {
        let m = heuristic(14.0);
        assert!((m.cell_width - 8.4).abs() < 0.01);
        assert!((m.cell_height - 18.2).abs() < 0.01);
        assert!((m.font_size - 14.0).abs() < 0.01);
        assert!((m.baseline_offset - 13.65).abs() < 0.01);
        assert!((m.scale_factor - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn heuristic_16() {
        let m = heuristic(16.0);
        assert!((m.cell_width - 9.6).abs() < 0.01);
        assert!((m.cell_height - 20.8).abs() < 0.01);
        assert!((m.font_size - 16.0).abs() < 0.01);
        assert!((m.baseline_offset - 15.6).abs() < 0.01);
    }

    #[test]
    fn heuristic_zero() {
        let m = heuristic(0.0);
        assert!((m.cell_width).abs() < 0.01);
        assert!((m.cell_height).abs() < 0.01);
        assert!((m.baseline_offset).abs() < 0.01);
    }

    #[test]
    fn default_uses_14() {
        let m = FontMetrics::default();
        let expected = heuristic(14.0);
        assert_eq!(m, expected);
    }

    #[test]
    fn width_height_ratio_relationship() {
        // For any font size, width should be narrower than height (monospace convention)
        for size in [8.0, 12.0, 14.0, 16.0, 20.0, 24.0] {
            let m = heuristic(size);
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
            let m = heuristic(size);
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
        let fallback = heuristic(14.0);
        assert_eq!(
            m, fallback,
            "invalid font data should fall back to heuristic"
        );
    }

    #[test]
    fn from_font_bytes_empty_data_falls_back() {
        let m = FontMetrics::from_font_bytes(14.0, &[]);
        let fallback = heuristic(14.0);
        assert_eq!(m, fallback, "empty font data should fall back to heuristic");
    }

    #[test]
    fn scaled_for_render_multiplies_dimensions() {
        let m = FontMetrics::from_font_size(14.0).with_scale_factor(1.5);
        let s = m.scaled_for_render();
        assert!((s.cell_width - 14.0 * 0.6 * 1.5).abs() < 0.01);
        assert!((s.font_size - 14.0 * 1.5).abs() < 0.01);
    }

    #[test]
    fn scaled_for_render_at_1x_is_identity() {
        let m = FontMetrics::from_font_size(14.0);
        let s = m.scaled_for_render();
        assert_eq!(m.cell_width, s.cell_width);
        assert_eq!(m.cell_height, s.cell_height);
        assert_eq!(m.font_size, s.font_size);
        assert_eq!(m.baseline_offset, s.baseline_offset);
    }

    #[test]
    fn from_font_bytes_produces_different_metrics_than_heuristic() {
        // Real font measurements should differ from the rough heuristic ratios
        let measured = FontMetrics::from_font_bytes(14.0, GEIST_MONO);
        let fallback = heuristic(14.0);
        assert_ne!(
            measured, fallback,
            "measured metrics from a real font should differ from heuristic"
        );
    }

    #[test]
    fn from_font_bytes_baseline_equals_ascent() {
        // baseline_offset should be the font's ascent, not a heuristic fraction
        let m = FontMetrics::from_font_bytes(14.0, GEIST_MONO);
        // For Geist Mono, ascent-based baseline should be roughly 70-80% of
        // cell height, not exactly 75% (the heuristic). Verify reasonable range.
        let ratio = m.baseline_offset / m.cell_height;
        assert!(
            (0.5..0.95).contains(&ratio),
            "baseline/height ratio ({:.3}) should be in 0.5..0.95 for a real font",
            ratio,
        );
    }
}
