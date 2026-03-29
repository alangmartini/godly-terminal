//! Status bar: process name, cwd, terminal dimensions.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::Rect;

pub struct StatusBar {
    pub process_name: String,
    pub terminal_size: (u16, u16),
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            process_name: String::new(),
            terminal_size: (24, 80),
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        // Background
        ui.fill(bar, colors::BG_BASE);

        // Top separator line
        ui.hline(bar.x, bar.y, bar.width, 1.0, colors::BG_SURFACE);

        // Process name (left-aligned)
        if !self.process_name.is_empty() {
            ui.text(
                text,
                &self.process_name,
                bar.x + 8.0,
                bar.y + 4.0,
                colors::FG_MUTED,
                colors::TRANSPARENT,
            );
        }

        // Terminal dimensions (right-aligned)
        let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
        // Approximate right alignment: each char ~8px wide
        let dims_width = dims.len() as f32 * 8.0;
        ui.text(
            text,
            &dims,
            bar.right() - dims_width - 8.0,
            bar.y + 4.0,
            colors::FG_MUTED,
            colors::TRANSPARENT,
        );
    }
}
