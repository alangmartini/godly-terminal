//! GPU renderer state management for the Tauri app.
//!
//! Wraps `godly-renderer::GpuRenderer` behind a `Mutex` for thread-safe access
//! from Tauri command handlers. The renderer is pre-warmed on a background thread
//! during app startup so it's ready before the first render request.

use std::sync::{Arc, Mutex};

use godly_renderer::GpuRenderer;
use godly_protocol::types::RichGridData;

/// Manages a GPU renderer instance that is pre-warmed on app startup.
///
/// The renderer is shared across all terminals. A background thread initializes
/// it during app startup (~500ms for GPU device + atlas), so the first render
/// request doesn't pay the cold-start cost. A `Mutex` serializes access since
/// `GpuRenderer` holds GPU device state that is not `Sync`.
///
/// The renderer is recreated when the DPI scale factor changes (e.g. window
/// moved to a different monitor).
pub struct GpuRendererManager {
    renderer: Mutex<Option<GpuRenderer>>,
    font_family: String,
    font_size: f32,
    current_dpr: Mutex<f32>,
}

impl GpuRendererManager {
    pub fn new(font_family: &str, font_size: f32) -> Self {
        Self {
            renderer: Mutex::new(None),
            font_family: font_family.to_string(),
            font_size,
            current_dpr: Mutex::new(0.0), // 0 = not yet initialized
        }
    }

    /// Pre-warm the GPU renderer on a background thread.
    ///
    /// Spawns a thread that initializes the wgpu device, glyph atlas, and
    /// render pipeline (~500ms total). This runs concurrently with app startup
    /// so the renderer is ready before the first terminal render request.
    pub fn warm(self: &Arc<Self>) {
        let mgr = Arc::clone(self);
        std::thread::Builder::new()
            .name("gpu-warm".into())
            .spawn(move || {
                let start = std::time::Instant::now();
                match mgr.ensure_renderer() {
                    Ok(()) => {
                        eprintln!(
                            "[gpu_renderer] Pre-warmed in {:.0}ms",
                            start.elapsed().as_secs_f64() * 1000.0
                        );
                    }
                    Err(e) => {
                        eprintln!("[gpu_renderer] Pre-warm failed: {e}");
                    }
                }
            })
            .expect("Failed to spawn GPU warm thread");
    }

    /// Ensure the GPU renderer is initialized at the given DPI scale.
    /// Recreates the renderer if the DPI changed.
    fn ensure_renderer_with_dpr(&self, dpr: f32) -> Result<(), String> {
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        let mut current_dpr = self.current_dpr.lock().map_err(|e| e.to_string())?;

        let dpr_changed = (*current_dpr - dpr).abs() > 0.01;

        if renderer.is_none() || dpr_changed {
            if dpr_changed && renderer.is_some() {
                eprintln!(
                    "[gpu_renderer] DPR changed {:.2} -> {:.2}, recreating atlas",
                    *current_dpr, dpr
                );
            }
            let scaled_size = self.font_size * dpr;
            *renderer = Some(
                GpuRenderer::new(&self.font_family, scaled_size)
                    .map_err(|e| format!("GPU renderer init failed: {e}"))?,
            );
            *current_dpr = dpr;
        }
        Ok(())
    }

    /// Ensure the GPU renderer is initialized (1x DPI fallback).
    fn ensure_renderer(&self) -> Result<(), String> {
        self.ensure_renderer_with_dpr(1.0)
    }

    /// Render a terminal grid to raw RGBA pixels.
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
        dpr: f32,
    ) -> Result<Vec<u8>, String> {
        self.ensure_renderer_with_dpr(dpr)?;
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
        dpr: f32,
    ) -> Result<Vec<u8>, String> {
        self.ensure_renderer_with_dpr(dpr)?;
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

    /// Check whether GPU rendering is available at the given DPI.
    pub fn is_available_with_dpr(&self, dpr: f32) -> bool {
        self.ensure_renderer_with_dpr(dpr).is_ok()
    }

    /// Check whether GPU rendering is available.
    pub fn is_available(&self) -> bool {
        self.ensure_renderer().is_ok()
    }
}
