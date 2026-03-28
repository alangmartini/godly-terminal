pub mod colors;
#[cfg(windows)]
pub mod directwrite_rasterizer;
pub mod font_loader;
pub mod font_metrics;
pub mod glyph_cache;
pub mod glyph_rasterizer;
pub mod pixel_renderer;
pub mod render_stats;
pub mod shader_surface;
mod surface;
pub mod swash_rasterizer;

pub use font_metrics::FontMetrics;
pub use surface::{GridPos, TerminalCanvas, TerminalCanvasState, DEFAULT_BG, DEFAULT_FG};
