pub mod color;
pub mod colors;
#[cfg(windows)]
pub mod directwrite_rasterizer;
pub mod font_loader;
pub mod font_metrics;
pub mod glyph_cache;
pub mod glyph_rasterizer;
pub mod atlas_shader;
pub mod atlas_vertex_builder;
pub mod glyph_atlas;
pub mod pixel_renderer;
pub mod render_stats;
pub mod shader_surface;
pub mod swash_rasterizer;

pub use color::Color;
pub use font_metrics::FontMetrics;

/// Grid position for selection rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPos {
    pub row: usize,
    pub col: usize,
}

/// Default terminal foreground (light gray).
pub const DEFAULT_FG: Color = Color { r: 0.8, g: 0.8, b: 0.8, a: 1.0 };

/// Default terminal background (near-black).
pub const DEFAULT_BG: Color = Color { r: 0.07, g: 0.07, b: 0.10, a: 1.0 };
