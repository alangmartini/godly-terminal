//! DirectWrite-based glyph rasterizer for ClearType-quality text on Windows.
//!
//! Uses Windows' native DirectWrite API to rasterize individual glyphs from
//! system fonts. Produces grayscale alpha bitmaps suitable for GPU texture
//! atlas upload.

use windows::core::*;
use windows::Win32::Foundation::{BOOL, E_FAIL};
use windows::Win32::Graphics::DirectWrite::*;

/// A rasterized glyph bitmap with positioning metadata.
pub struct RasterizedGlyphDW {
    /// Grayscale alpha bitmap (1 byte per pixel, row-major).
    pub alpha: Vec<u8>,
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
/// to grayscale alpha bitmaps using Windows' native text rendering.
pub struct DirectWriteRasterizer {
    factory: IDWriteFactory,
    font_face: Option<IDWriteFontFace>,
    /// Stored for future ClearType subpixel blending (`GetAlphaBlendParams`).
    #[allow(dead_code)]
    rendering_params: IDWriteRenderingParams,
    /// DPI scale factor (pixels per DIP). At 100% scaling this is 1.0;
    /// at 150% it is 1.5, etc. Passed to `CreateGlyphRunAnalysis` so that
    /// DirectWrite rasterizes glyphs at the physical pixel resolution.
    scale_factor: f32,
}

impl DirectWriteRasterizer {
    /// Create a new rasterizer, initializing the DirectWrite factory.
    ///
    /// The DPI scale factor defaults to 1.0. Call [`set_scale_factor`] to
    /// update it when the window's DPI changes.
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let rendering_params = factory.CreateRenderingParams()?;
            Ok(Self {
                factory,
                font_face: None,
                rendering_params,
                scale_factor: 1.0,
            })
        }
    }

    /// Update the DPI scale factor (pixels per DIP).
    ///
    /// This should be called whenever the window moves to a display with a
    /// different DPI. After changing the scale factor the caller should
    /// invalidate the glyph cache, since cached bitmaps were rasterized at
    /// the old DPI.
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
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
            Ok(())
        }
    }

    /// Rasterize a single glyph at the given font size.
    ///
    /// Returns `Ok(None)` if the font does not contain a glyph for the
    /// requested character (glyph index 0 / .notdef).
    ///
    /// Uses `DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC` with
    /// `DWRITE_TEXTURE_CLEARTYPE_3x1` for ClearType-quality rendering,
    /// then averages the RGB subpixel channels into a single grayscale
    /// alpha value per pixel.
    pub fn rasterize_glyph(
        &self,
        ch: char,
        font_size_px: f32,
    ) -> windows::core::Result<Option<RasterizedGlyphDW>> {
        let font_face = self
            .font_face
            .as_ref()
            .ok_or(windows::core::Error::from(E_FAIL))?;

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
                self.scale_factor, // pixels per DIP — actual OS scale factor
                None,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0, // baseline origin
            )?;

            let bounds = analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)?;

            let width = (bounds.right - bounds.left) as u32;
            let height = (bounds.bottom - bounds.top) as u32;

            if width == 0 || height == 0 {
                return Ok(Some(RasterizedGlyphDW {
                    alpha: vec![],
                    width: 0,
                    height: 0,
                    bearing_x: 0,
                    bearing_y: 0,
                    advance: metrics[0].advanceWidth as f32 * scale,
                }));
            }

            let mut rgb = vec![0u8; (width * height * 3) as usize];
            analysis.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut rgb)?;

            let alpha: Vec<u8> = rgb
                .chunks_exact(3)
                .map(|rgb| {
                    let sum = rgb[0] as u16 + rgb[1] as u16 + rgb[2] as u16;
                    (sum / 3) as u8
                })
                .collect();

            let advance = metrics[0].advanceWidth as f32 * scale;

            Ok(Some(RasterizedGlyphDW {
                alpha,
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
        assert!(!glyph.alpha.is_empty());
        assert!(glyph.advance > 0.0);
        // Alpha buffer size must match dimensions
        assert_eq!(glyph.alpha.len(), (glyph.width * glyph.height) as usize);
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
}
