pub mod colors;
#[cfg(windows)]
pub mod directwrite_rasterizer;
pub mod font_metrics;
pub mod glyph_cache;
pub mod glyph_rasterizer;
pub mod font_loader;
pub mod pixel_renderer;
pub mod render_stats;
pub mod swash_rasterizer;
mod surface;

pub use font_metrics::FontMetrics;
pub use surface::{GridPos, TerminalCanvas, TerminalCanvasState, DEFAULT_BG, DEFAULT_FG};
