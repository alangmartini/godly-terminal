//! GPU renderer state management for the Tauri app.
//!
//! Wraps `godly-renderer::GpuRenderer` behind a `Mutex` for thread-safe access
//! from Tauri command handlers. The renderer is lazily initialized on first use
//! so app startup isn't blocked by GPU adapter enumeration.

use std::sync::Mutex;

use godly_renderer::GpuRenderer;
use godly_protocol::types::RichGridData;

/// Manages a lazily-initialized GPU renderer instance.
///
/// The renderer is shared across all terminals. It is initialized on first
/// render request and cached for subsequent calls. A `Mutex` serializes
/// access since `GpuRenderer` holds GPU device state that is not `Sync`.
pub struct GpuRendererManager {
    renderer: Mutex<Option<GpuRenderer>>,
    font_family: String,
    font_size: f32,
}

impl GpuRendererManager {
    pub fn new(font_family: &str, font_size: f32) -> Self {
        Self {
            renderer: Mutex::new(None),
            font_family: font_family.to_string(),
            font_size,
        }
    }

    /// Ensure the GPU renderer is initialized, creating it if needed.
    fn ensure_renderer(&self) -> Result<(), String> {
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        if renderer.is_none() {
            *renderer = Some(
                GpuRenderer::new(&self.font_family, self.font_size)
                    .map_err(|e| format!("GPU renderer init failed: {e}"))?,
            );
        }
        Ok(())
    }

    /// Render a terminal grid to raw RGBA pixels.
    ///
    /// Returns `(width, height, rgba_bytes)`.
    pub fn render_terminal(
        &self,
        grid: &RichGridData,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        self.ensure_renderer()?;
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        renderer
            .as_mut()
            .unwrap()
            .render_to_pixels(grid)
            .map_err(|e| format!("GPU render failed: {e}"))
    }

    /// Render a terminal grid to PNG-encoded bytes.
    pub fn render_terminal_png(
        &self,
        grid: &RichGridData,
    ) -> Result<Vec<u8>, String> {
        self.ensure_renderer()?;
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        renderer
            .as_mut()
            .unwrap()
            .render_to_png(grid)
            .map_err(|e| format!("GPU render failed: {e}"))
    }

    /// Render a terminal grid to raw RGBA bytes with a dimensions header.
    ///
    /// Format: `[width: u32 LE][height: u32 LE][rgba_pixels...]`
    pub fn render_terminal_raw(
        &self,
        grid: &RichGridData,
    ) -> Result<Vec<u8>, String> {
        self.ensure_renderer()?;
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        let (width, height, pixels) = renderer
            .as_mut()
            .unwrap()
            .render_to_pixels(grid)
            .map_err(|e| format!("GPU render failed: {e}"))?;

        let mut result = Vec::with_capacity(8 + pixels.len());
        result.extend_from_slice(&width.to_le_bytes());
        result.extend_from_slice(&height.to_le_bytes());
        result.extend_from_slice(&pixels);
        Ok(result)
    }

    /// Get the cell size (width, height) in pixels from the GPU renderer.
    pub fn cell_size(&self) -> Result<(f64, f64), String> {
        self.ensure_renderer()?;
        let renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        let (w, h) = renderer.as_ref().unwrap().cell_size();
        Ok((w as f64, h as f64))
    }

    /// Check whether GPU rendering is available (i.e. a renderer can be created).
    pub fn is_available(&self) -> bool {
        self.ensure_renderer().is_ok()
    }
}
