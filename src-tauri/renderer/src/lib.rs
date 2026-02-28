//! GPU-accelerated terminal renderer for Godly Terminal.
//!
//! This crate renders `RichGridData` terminal snapshots (from `godly-protocol`)
//! to raw RGBA pixel buffers or PNG images using wgpu. It is designed for
//! headless/offscreen rendering -- no window or surface is needed.
//!
//! # Architecture
//!
//! - **GpuDevice** — wgpu adapter/device/queue initialization (headless).
//! - **GlyphAtlas** — Font loading (cosmic-text), glyph rasterization, texture atlas management.
//! - **RenderPipeline** — wgpu render pipeline with WGSL shaders for textured cell quads.
//! - **GpuRenderer** — Main API: `RichGridData` → pixel buffer or PNG.
//!
//! # Usage
//!
//! ```no_run
//! use godly_renderer::{GpuRenderer, TerminalTheme};
//! use godly_protocol::types::RichGridData;
//!
//! let mut renderer = GpuRenderer::new("Cascadia Code", 14.0).unwrap();
//! renderer.set_theme(TerminalTheme::default());
//!
//! // Given a RichGridData from the daemon:
//! // let (width, height, pixels) = renderer.render_to_pixels(&grid).unwrap();
//! // let png_bytes = renderer.render_to_png(&grid).unwrap();
//! ```

pub mod atlas;
pub mod color;
pub mod device;
pub mod pipeline;
pub mod renderer;
pub mod theme;

pub use color::{parse_hex_color, resolve_cell_colors};
pub use device::{enumerate_gpu_adapters, GpuAdapterInfo};
pub use renderer::GpuRenderer;
pub use theme::TerminalTheme;

/// Expose internal construction phases for benchmarking cold-start costs.
/// Not intended for production use — use `GpuRenderer::new()` instead.
pub mod cold_start_phases {
    use crate::atlas::GlyphAtlas;
    use crate::device::GpuDevice;
    use crate::pipeline::RenderPipeline;
    use crate::GpuError;

    pub fn create_device() -> Result<GpuDevice, GpuError> {
        GpuDevice::new()
    }

    pub fn create_atlas(font_family: &str, font_size: f32) -> GlyphAtlas {
        GlyphAtlas::new(font_family, font_size)
    }

    pub fn create_pipeline(device: &GpuDevice) -> RenderPipeline {
        RenderPipeline::new(&device.device, wgpu::TextureFormat::Rgba8Unorm)
    }
}

use std::fmt;

/// Errors that can occur during GPU rendering.
#[derive(Debug)]
pub enum GpuError {
    /// No suitable GPU adapter was found on the system.
    NoAdapter,
    /// Failed to create the GPU device.
    DeviceError(String),
    /// An error occurred during rendering.
    RenderError(String),
    /// Image encoding/decoding failed.
    ImageError,
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "No suitable GPU adapter found"),
            GpuError::DeviceError(msg) => write!(f, "Failed to create GPU device: {}", msg),
            GpuError::RenderError(msg) => write!(f, "Render error: {}", msg),
            GpuError::ImageError => write!(f, "Image encoding error"),
        }
    }
}

impl std::error::Error for GpuError {}
