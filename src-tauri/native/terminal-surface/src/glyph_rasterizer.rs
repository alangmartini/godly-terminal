/// Result of rasterizing a single glyph.
pub struct RasterizedGlyph {
    /// Alpha mask pixels (one byte per pixel, 8-bit coverage).
    pub alpha: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Horizontal offset from glyph origin to left edge of bitmap.
    pub bearing_x: i32,
    /// Vertical offset from baseline to top edge of bitmap (positive = above baseline).
    pub bearing_y: i32,
    /// Horizontal advance width in pixels.
    pub advance: f32,
}

/// Measured font metrics from actual font data.
pub struct MeasuredFontMetrics {
    pub ascent: f32,
    pub descent: f32, // positive value (abs of negative descent)
    pub leading: f32,
    pub average_advance: f32,
    pub is_monospace: bool,
}

/// Trait for glyph rasterization backends.
pub trait GlyphRasterizer {
    fn rasterize(
        &mut self,
        ch: char,
        font_size_px: f32,
        bold: bool,
        italic: bool,
    ) -> Option<RasterizedGlyph>;
    fn measure(&mut self, font_size_px: f32) -> MeasuredFontMetrics;
    fn has_glyph(&self, ch: char) -> bool;
    fn load_font(&mut self, data: &[u8], index: u32) -> bool;
}
