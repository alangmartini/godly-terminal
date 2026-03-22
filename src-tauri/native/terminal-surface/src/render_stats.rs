use std::time::Duration;

/// Per-frame render statistics for the PixelRenderer pipeline.
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    /// Time spent filling the background.
    pub bg_fill: Duration,
    /// Time spent on glyph lookup + rasterization + blitting.
    pub glyph_phase: Duration,
    /// Time spent drawing the cursor.
    pub cursor_phase: Duration,
    /// Time spent drawing selection overlay.
    pub selection_phase: Duration,
    /// Total render() wall time.
    pub total: Duration,
    /// Number of non-empty, non-continuation cells rendered.
    pub cells_rendered: u32,
    /// Number of rows processed.
    pub rows_rendered: u32,
}

impl RenderStats {
    pub fn total_us(&self) -> u64 {
        self.total.as_micros() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_stats_default_is_zero() {
        let stats = RenderStats::default();
        assert_eq!(stats.bg_fill, Duration::ZERO);
        assert_eq!(stats.glyph_phase, Duration::ZERO);
        assert_eq!(stats.cursor_phase, Duration::ZERO);
        assert_eq!(stats.selection_phase, Duration::ZERO);
        assert_eq!(stats.total, Duration::ZERO);
        assert_eq!(stats.cells_rendered, 0);
        assert_eq!(stats.rows_rendered, 0);
    }

    #[test]
    fn total_us_converts_correctly() {
        let stats = RenderStats {
            total: Duration::from_micros(12345),
            ..Default::default()
        };
        assert_eq!(stats.total_us(), 12345);
    }
}
