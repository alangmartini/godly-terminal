//! DirectWrite-based glyph rasterizer for ClearType-quality text on Windows.
//!
//! Uses Windows' native DirectWrite API to rasterize individual glyphs from
//! system fonts. Produces ClearType subpixel RGB bitmaps (3 bytes per pixel)
//! for sharp LCD text rendering.

use windows::core::*;
use windows::Win32::Foundation::{BOOL, E_FAIL};
use windows::Win32::Graphics::DirectWrite::*;

use crate::glyph_rasterizer::{GlyphFormat, GlyphRasterizer, MeasuredFontMetrics, RasterizedGlyph};

/// A rasterized glyph bitmap with positioning metadata.
pub struct RasterizedGlyphDW {
    /// ClearType subpixel bitmap (3 bytes per pixel: R, G, B coverage, row-major).
    pub rgb: Vec<u8>,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Horizontal bearing (offset from pen position to left edge of bitmap).
    pub bearing_x: i32,
    /// Vertical bearing (offset from baseline to top edge of bitmap).
    pub bearing_y: i32,
    /// Horizontal advance width in pixels.
    pub advance: f32,
}

/// Font metrics measured from a loaded DirectWrite font face.
pub struct DWriteFontMetrics {
    /// Distance from baseline to top of character em square.
    pub ascent: f32,
    /// Distance from baseline to bottom of character em square.
    pub descent: f32,
    /// Extra line spacing (line gap).
    pub leading: f32,
    /// Average character advance width (measured from '0').
    pub average_advance: f32,
}

/// DirectWrite-based glyph rasterizer.
///
/// Loads system fonts by family name and rasterizes individual glyphs
/// to ClearType subpixel RGB bitmaps using Windows' native text rendering.
pub struct DirectWriteRasterizer {
    factory: IDWriteFactory,
    font_face: Option<IDWriteFontFace>,
    medium_face: Option<IDWriteFontFace>,
    semibold_face: Option<IDWriteFontFace>,
    bold_face: Option<IDWriteFontFace>,
    italic_face: Option<IDWriteFontFace>,
    bold_italic_face: Option<IDWriteFontFace>,
    font_family_name: String,
    /// Stored for future ClearType subpixel blending (`GetAlphaBlendParams`).
    #[allow(dead_code)]
    rendering_params: IDWriteRenderingParams,
    output_mode: DirectWriteOutputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectWriteOutputMode {
    SubpixelRgb,
    GrayscaleAlpha,
}

impl DirectWriteRasterizer {
    /// Create a new rasterizer, initializing the DirectWrite factory.
    pub fn new() -> windows::core::Result<Self> {
        Self::with_output_mode(DirectWriteOutputMode::SubpixelRgb)
    }

    /// Create a rasterizer that emits grayscale alpha coverage instead of
    /// ClearType subpixel coverage. This is intended for screenshot-parity
    /// paths where the target is a browser screenshot rather than a live LCD.
    pub fn new_grayscale() -> windows::core::Result<Self> {
        Self::with_output_mode(DirectWriteOutputMode::GrayscaleAlpha)
    }

    fn with_output_mode(output_mode: DirectWriteOutputMode) -> windows::core::Result<Self> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let rendering_params = factory.CreateRenderingParams()?;
            Ok(Self {
                factory,
                font_face: None,
                medium_face: None,
                semibold_face: None,
                bold_face: None,
                italic_face: None,
                bold_italic_face: None,
                font_family_name: String::new(),
                rendering_params,
                output_mode,
            })
        }
    }

    /// Update the DPI scale factor.
    ///
    /// No-op: the pixel renderer already passes physical-pixel font sizes,
    /// so DirectWrite uses `pixelsPerDip = 1.0`. The caller should still
    /// invalidate the glyph cache after DPI changes, since cached bitmaps
    /// were rasterized at the old physical size.
    pub fn set_scale_factor(&mut self, _scale_factor: f32) {}

    /// Select the appropriate font face for the given weight/italic combination.
    ///
    /// Falls back to the nearest available weight, then to the regular face.
    fn face_for_weight(&self, weight: u16, italic: bool) -> Option<&IDWriteFontFace> {
        if italic && weight >= 700 {
            return self
                .bold_italic_face
                .as_ref()
                .or(self.bold_face.as_ref())
                .or(self.font_face.as_ref());
        }
        if italic {
            return self.italic_face.as_ref().or(self.font_face.as_ref());
        }
        match weight {
            700.. => self.bold_face.as_ref().or(self.font_face.as_ref()),
            600..=699 => self
                .semibold_face
                .as_ref()
                .or(self.bold_face.as_ref())
                .or(self.font_face.as_ref()),
            500..=599 => self
                .medium_face
                .as_ref()
                .or(self.font_face.as_ref()),
            _ => self.font_face.as_ref(),
        }
    }

    /// Load a system font by family name (e.g. "Consolas", "Cascadia Code").
    ///
    /// Selects the Regular weight/stretch/style variant. The loaded font face
    /// is stored for subsequent `rasterize_glyph` and `measure_font` calls.
    pub fn load_system_font(&mut self, family_name: &str) -> windows::core::Result<()> {
        unsafe {
            let mut collection: Option<IDWriteFontCollection> = None;
            self.factory
                .GetSystemFontCollection(&mut collection, false)?;
            let collection = collection.ok_or(windows::core::Error::from(E_FAIL))?;

            let mut index = 0u32;
            let mut exists = BOOL::default();
            let family_wide: Vec<u16> = family_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let family_pcwstr = PCWSTR(family_wide.as_ptr());
            collection.FindFamilyName(family_pcwstr, &mut index, &mut exists)?;

            if !exists.as_bool() {
                return Err(windows::core::Error::from(E_FAIL));
            }

            let font_family = collection.GetFontFamily(index)?;
            let font = font_family.GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )?;
            self.font_face = Some(font.CreateFontFace()?);
            self.font_family_name = family_name.to_string();

            // Load medium (500) variant (optional)
            self.medium_face = font_family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT(500),
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )
                .ok()
                .and_then(|f| f.CreateFontFace().ok());

            // Load semibold (600) variant (optional)
            self.semibold_face = font_family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT(600),
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )
                .ok()
                .and_then(|f| f.CreateFontFace().ok());

            // Load bold variant (optional — fall back to regular if unavailable)
            self.bold_face = font_family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )
                .ok()
                .and_then(|f| f.CreateFontFace().ok());

            // Load italic variant
            self.italic_face = font_family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_REGULAR,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_ITALIC,
                )
                .ok()
                .and_then(|f| f.CreateFontFace().ok());

            // Load bold-italic variant
            self.bold_italic_face = font_family
                .GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_ITALIC,
                )
                .ok()
                .and_then(|f| f.CreateFontFace().ok());

            Ok(())
        }
    }

    /// Rasterize a single glyph at the given font size.
    ///
    /// Returns `Ok(None)` if the font does not contain a glyph for the
    /// requested character (glyph index 0 / .notdef).
    ///
    /// Uses `DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC` with
    /// `DWRITE_TEXTURE_CLEARTYPE_3x1` for ClearType subpixel rendering.
    /// The returned bitmap contains 3 bytes per pixel (R, G, B coverage)
    /// preserving the full subpixel information for LCD displays.
    pub fn rasterize_glyph(
        &self,
        ch: char,
        font_size_px: f32,
    ) -> windows::core::Result<Option<RasterizedGlyphDW>> {
        // Legacy helper: callers of this API expect raw ClearType RGB coverage.
        // The configurable grayscale path is exposed through `GlyphRasterizer`.
        let font_face = self
            .font_face
            .as_ref()
            .ok_or(windows::core::Error::from(E_FAIL))?;
        let glyph = self.rasterize_with_face_mode(
            font_face,
            ch,
            font_size_px,
            DirectWriteOutputMode::SubpixelRgb,
        )?;

        Ok(glyph.map(|glyph| RasterizedGlyphDW {
            rgb: glyph.data,
            width: glyph.width,
            height: glyph.height,
            bearing_x: glyph.bearing_x,
            bearing_y: glyph.bearing_y,
            advance: glyph.advance,
        }))
    }

    /// Core rasterization logic using a specific font face.
    fn rasterize_with_face_mode(
        &self,
        font_face: &IDWriteFontFace,
        ch: char,
        font_size_px: f32,
        output_mode: DirectWriteOutputMode,
    ) -> windows::core::Result<Option<RasterizedGlyph>> {
        unsafe {
            let codepoints = [ch as u32];
            let mut glyph_indices = [0u16; 1];
            font_face.GetGlyphIndices(codepoints.as_ptr(), 1, glyph_indices.as_mut_ptr())?;

            if glyph_indices[0] == 0 {
                return Ok(None);
            }

            let mut metrics = [DWRITE_GLYPH_METRICS::default(); 1];
            font_face.GetDesignGlyphMetrics(
                glyph_indices.as_ptr(),
                1,
                metrics.as_mut_ptr(),
                false,
            )?;

            let scale = Self::design_to_px_scale(font_face, font_size_px);

            let glyph_run = DWRITE_GLYPH_RUN {
                fontFace: std::mem::ManuallyDrop::new(Some(font_face.clone())),
                fontEmSize: font_size_px,
                glyphCount: 1,
                glyphIndices: glyph_indices.as_ptr(),
                glyphAdvances: std::ptr::null(),
                glyphOffsets: std::ptr::null(),
                isSideways: false.into(),
                bidiLevel: 0,
            };

            let analysis = self.factory.CreateGlyphRunAnalysis(
                &glyph_run,
                1.0, // font_size_px is already in physical pixels (scaled by pixel_renderer)
                None,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0, // baseline origin
            )?;

            // DirectWrite only exposes ClearType 3x1 or aliased 1x1 texture
            // extraction here. For screenshot/browser-style grayscale AA we
            // still request ClearType coverage, then collapse the three
            // subpixel channels into a single alpha coverage value.
            let texture_type = DWRITE_TEXTURE_CLEARTYPE_3x1;
            let bounds = analysis.GetAlphaTextureBounds(texture_type)?;

            let width = (bounds.right - bounds.left) as u32;
            let height = (bounds.bottom - bounds.top) as u32;
            let format = match output_mode {
                DirectWriteOutputMode::SubpixelRgb => GlyphFormat::SubpixelRgb,
                DirectWriteOutputMode::GrayscaleAlpha => GlyphFormat::Alpha,
            };

            if width == 0 || height == 0 {
                return Ok(Some(RasterizedGlyph {
                    data: vec![],
                    format,
                    width: 0,
                    height: 0,
                    bearing_x: 0,
                    bearing_y: 0,
                    advance: metrics[0].advanceWidth as f32 * scale,
                }));
            }

            let mut texture_data = vec![0u8; (width * height * 3) as usize];
            analysis.CreateAlphaTexture(texture_type, &bounds, &mut texture_data)?;
            let data = match output_mode {
                DirectWriteOutputMode::SubpixelRgb => texture_data,
                DirectWriteOutputMode::GrayscaleAlpha => collapse_cleartype_to_alpha(&texture_data),
            };

            let advance = metrics[0].advanceWidth as f32 * scale;

            Ok(Some(RasterizedGlyph {
                data,
                format,
                width,
                height,
                bearing_x: bounds.left,
                bearing_y: -bounds.top, // DW uses top-down; convert to baseline-relative
                advance,
            }))
        }
    }

    /// Measure font metrics at the given size.
    ///
    /// Returns ascent, descent, leading, and average advance (measured from '0').
    pub fn measure_font(&self, font_size_px: f32) -> Option<DWriteFontMetrics> {
        let font_face = self.font_face.as_ref()?;
        unsafe {
            let mut fm = DWRITE_FONT_METRICS::default();
            font_face.GetMetrics(&mut fm);
            let scale = font_size_px / fm.designUnitsPerEm as f32;
            Some(DWriteFontMetrics {
                ascent: fm.ascent as f32 * scale,
                descent: fm.descent as f32 * scale,
                leading: fm.lineGap as f32 * scale,
                average_advance: self
                    .measure_advance('0', font_size_px)
                    .unwrap_or(font_size_px * 0.6),
            })
        }
    }

    /// Compute the scale factor from font design units to pixels.
    unsafe fn design_to_px_scale(font_face: &IDWriteFontFace, font_size_px: f32) -> f32 {
        let mut fm = DWRITE_FONT_METRICS::default();
        font_face.GetMetrics(&mut fm);
        font_size_px / fm.designUnitsPerEm as f32
    }

    fn measure_advance(&self, ch: char, font_size_px: f32) -> Option<f32> {
        let font_face = self.font_face.as_ref()?;
        unsafe {
            let codepoints = [ch as u32];
            let mut indices = [0u16; 1];
            font_face
                .GetGlyphIndices(codepoints.as_ptr(), 1, indices.as_mut_ptr())
                .ok()?;

            let mut metrics = [DWRITE_GLYPH_METRICS::default(); 1];
            font_face
                .GetDesignGlyphMetrics(indices.as_ptr(), 1, metrics.as_mut_ptr(), false)
                .ok()?;

            let scale = Self::design_to_px_scale(font_face, font_size_px);
            Some(metrics[0].advanceWidth as f32 * scale)
        }
    }
}

impl GlyphRasterizer for DirectWriteRasterizer {
    fn load_font(&mut self, _data: &[u8], _index: u32) -> bool {
        // DirectWrite loads fonts by family name via load_system_font(),
        // not from raw bytes. Report whether a font is already loaded.
        self.font_face.is_some()
    }

    fn rasterize(
        &mut self,
        ch: char,
        font_size_px: f32,
        weight: u16,
        italic: bool,
    ) -> Option<RasterizedGlyph> {
        let face = self.face_for_weight(weight, italic)?.clone();
        self.rasterize_with_face_mode(&face, ch, font_size_px, self.output_mode)
            .ok()?
    }

    fn measure(&mut self, font_size_px: f32) -> MeasuredFontMetrics {
        match self.measure_font(font_size_px) {
            Some(m) => MeasuredFontMetrics {
                ascent: m.ascent,
                descent: m.descent,
                leading: m.leading,
                average_advance: m.average_advance,
                is_monospace: true,
            },
            None => MeasuredFontMetrics {
                ascent: font_size_px * 0.8,
                descent: font_size_px * 0.2,
                leading: 0.0,
                average_advance: font_size_px * 0.6,
                is_monospace: true,
            },
        }
    }

    fn has_glyph(&self, ch: char) -> bool {
        let Some(font_face) = self.font_face.as_ref() else {
            return false;
        };
        unsafe {
            let codepoints = [ch as u32];
            let mut glyph_indices = [0u16; 1];
            if font_face
                .GetGlyphIndices(codepoints.as_ptr(), 1, glyph_indices.as_mut_ptr())
                .is_err()
            {
                return false;
            }
            glyph_indices[0] != 0
        }
    }
}

fn collapse_cleartype_to_alpha(rgb: &[u8]) -> Vec<u8> {
    let mut alpha = Vec::with_capacity(rgb.len() / 3);
    for pixel in rgb.chunks_exact(3) {
        let coverage = (pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3;
        alpha.push(coverage as u8);
    }
    alpha
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_factory() {
        let _rasterizer = DirectWriteRasterizer::new().expect("factory creation should succeed");
    }

    #[test]
    fn load_consolas() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        rasterizer
            .load_system_font("Consolas")
            .expect("Consolas should exist on Windows");
    }

    #[test]
    fn load_nonexistent_font_fails() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        let result = rasterizer.load_system_font("NonExistentFont_XYZ_999");
        assert!(result.is_err(), "loading a nonexistent font should fail");
    }

    #[test]
    fn rasterize_ascii_a() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        rasterizer.load_system_font("Consolas").unwrap();
        let glyph = rasterizer
            .rasterize_glyph('A', 14.0)
            .unwrap()
            .expect("'A' should exist in Consolas");
        assert!(glyph.width > 0);
        assert!(glyph.height > 0);
        assert!(!glyph.rgb.is_empty());
        assert!(glyph.advance > 0.0);
        // RGB buffer size must match dimensions (3 bytes per pixel)
        assert_eq!(glyph.rgb.len(), (glyph.width * glyph.height * 3) as usize);
    }

    #[test]
    fn rasterize_without_font_fails() {
        let rasterizer = DirectWriteRasterizer::new().unwrap();
        let result = rasterizer.rasterize_glyph('A', 14.0);
        assert!(
            result.is_err(),
            "rasterizing without a loaded font should fail"
        );
    }

    #[test]
    fn measure_font_metrics() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        rasterizer.load_system_font("Consolas").unwrap();
        let metrics = rasterizer.measure_font(14.0).expect("should measure font");
        assert!(metrics.ascent > 0.0, "ascent should be positive");
        assert!(metrics.descent > 0.0, "descent should be positive");
        assert!(
            metrics.average_advance > 0.0,
            "average_advance should be positive"
        );
    }

    #[test]
    fn measure_font_without_load_returns_none() {
        let rasterizer = DirectWriteRasterizer::new().unwrap();
        assert!(
            rasterizer.measure_font(14.0).is_none(),
            "measure_font without a loaded font should return None"
        );
    }

    #[test]
    fn missing_glyph_returns_none() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        rasterizer.load_system_font("Consolas").unwrap();
        // Private-use area character unlikely to be in Consolas
        let glyph = rasterizer.rasterize_glyph('\u{F0000}', 14.0).unwrap();
        assert!(
            glyph.is_none(),
            "rare private-use character should not be in Consolas"
        );
    }

    #[test]
    fn rasterize_multiple_sizes() {
        let mut rasterizer = DirectWriteRasterizer::new().unwrap();
        rasterizer.load_system_font("Consolas").unwrap();

        let small = rasterizer
            .rasterize_glyph('A', 10.0)
            .unwrap()
            .expect("'A' at 10px");
        let large = rasterizer
            .rasterize_glyph('A', 24.0)
            .unwrap()
            .expect("'A' at 24px");

        assert!(
            large.width >= small.width,
            "larger font size should produce wider or equal glyph"
        );
        assert!(
            large.height >= small.height,
            "larger font size should produce taller or equal glyph"
        );
        assert!(
            large.advance > small.advance,
            "larger font size should have greater advance"
        );
    }

    // --- GlyphRasterizer trait tests ---

    #[test]
    fn trait_rasterize_produces_subpixel_rgb() {
        let mut rast = DirectWriteRasterizer::new().unwrap();
        rast.load_system_font("Consolas").unwrap();
        let glyph = rast
            .rasterize('A', 14.0, 400, false)
            .expect("'A' should rasterize via trait");
        assert_eq!(glyph.format, GlyphFormat::SubpixelRgb);
        assert!(glyph.width > 0);
        assert!(glyph.height > 0);
        // SubpixelRgb: 3 bytes per pixel
        assert_eq!(
            glyph.data.len(),
            (glyph.width * glyph.height * 3) as usize,
            "data length must be width * height * 3 for SubpixelRgb"
        );
    }

    #[test]
    fn trait_rasterize_bold() {
        let mut rast = DirectWriteRasterizer::new().unwrap();
        rast.load_system_font("Consolas").unwrap();
        let normal = rast
            .rasterize('A', 14.0, 400, false)
            .expect("normal 'A' should rasterize");
        let bold = rast
            .rasterize('A', 14.0, 700, false)
            .expect("bold 'A' should rasterize");
        assert!(normal.width > 0);
        assert!(bold.width > 0);
        assert_eq!(normal.format, GlyphFormat::SubpixelRgb);
        assert_eq!(bold.format, GlyphFormat::SubpixelRgb);
    }

    #[test]
    fn trait_has_glyph() {
        let mut rast = DirectWriteRasterizer::new().unwrap();
        rast.load_system_font("Consolas").unwrap();
        assert!(rast.has_glyph('A'), "ASCII 'A' should be present");
        assert!(rast.has_glyph('z'), "ASCII 'z' should be present");
        assert!(rast.has_glyph('0'), "ASCII '0' should be present");
        assert!(
            !rast.has_glyph('\u{F0000}'),
            "private-use character should be absent"
        );
    }

    #[test]
    fn trait_measure() {
        let mut rast = DirectWriteRasterizer::new().unwrap();
        rast.load_system_font("Consolas").unwrap();
        let m = rast.measure(14.0);
        assert!(m.ascent > 0.0, "ascent should be positive");
        assert!(m.descent > 0.0, "descent should be positive");
        assert!(m.average_advance > 0.0, "advance should be positive");
        assert!(m.is_monospace, "Consolas should report as monospace");
    }

    #[test]
    fn grayscale_mode_emits_alpha_glyphs() {
        let mut rast = DirectWriteRasterizer::new_grayscale().unwrap();
        rast.load_system_font("Consolas").unwrap();
        let glyph = rast
            .rasterize('A', 14.0, 400, false)
            .expect("'A' should rasterize in grayscale mode");
        assert_eq!(glyph.format, GlyphFormat::Alpha);
        assert_eq!(glyph.data.len(), (glyph.width * glyph.height) as usize);
        assert!(
            glyph.data.iter().any(|&value| value > 0 && value < 255),
            "grayscale mode should preserve antialiased coverage, not just bilevel pixels"
        );
    }

    #[test]
    fn cleartype_alpha_collapse_averages_subpixel_coverage() {
        let alpha = collapse_cleartype_to_alpha(&[30, 120, 240, 10, 10, 10]);
        assert_eq!(alpha, vec![130, 10]);
    }
}
