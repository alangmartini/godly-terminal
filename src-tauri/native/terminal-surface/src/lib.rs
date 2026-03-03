pub mod colors;
pub mod font_metrics;
mod surface;

pub use font_metrics::FontMetrics;
pub use surface::{GridPos, TerminalCanvas, TerminalCanvasState, DEFAULT_BG, DEFAULT_FG};
