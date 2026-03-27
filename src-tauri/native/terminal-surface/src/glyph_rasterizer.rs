/// Distinguishes between grayscale and subpixel (ClearType) glyph data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphFormat {
    /// Single-channel grayscale alpha (1 byte per pixel).
    Alpha,
    /// Three-channel ClearType subpixel data (3 bytes per pixel: R, G, B).
    SubpixelRgb,
}

/// Result of rasterizing a single glyph.
pub struct RasterizedGlyph {
    /// Glyph bitmap data. Layout depends on `format`:
    /// - `Alpha`: 1 byte per pixel (8-bit coverage).
    /// - `SubpixelRgb`: 3 bytes per pixel (R, G, B coverage values).
    pub data: Vec<u8>,
    /// Format of the data in `data`.
    pub format: GlyphFormat,
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
    /// Update the DPI scale factor. Default is a no-op for backends that
    /// don't need it (e.g. Swash which works in pixel units directly).
    fn set_scale_factor(&mut self, _scale: f32) {}
}
