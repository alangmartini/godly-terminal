//! Status bar: working directory, git branch, terminal dimensions.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::Rect;

pub struct StatusBar {
    pub process_name: String,
    pub cwd: String,
    pub git_branch: String,
    pub terminal_size: (u16, u16),
    pub sidebar_width: f32,
    pub git_diff_summary: String,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            process_name: String::new(),
            cwd: String::new(),
            git_branch: String::new(),
            terminal_size: (24, 80),
            sidebar_width: 0.0,
            git_diff_summary: String::new(),
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;

        // Background — slightly raised surface for visibility
        let sidebar_bg = colors::BG_DARK;
        let content_bg = colors::BG_SURFACE;
        ui.fill(bar, content_bg);

        // Sidebar portion of status bar (darker background)
        if self.sidebar_width > 0.0 {
            let sidebar_status = Rect {
                x: bar.x,
                y: bar.y,
                width: self.sidebar_width,
                height: bar.height,
            };
            ui.fill(sidebar_status, sidebar_bg);
            // Right border on sidebar section
            ui.vline(self.sidebar_width - 1.0, bar.y, bar.height, 1.0, colors::BORDER);
        }

        // Top separator line
        ui.hline(bar.x, bar.y, bar.width, 1.0, colors::BORDER);

        let y_center = bar.y + (bar.height - ch) / 2.0;

        // --- Sidebar section: mode indicator with dot ---
        if self.sidebar_width > 0.0 {
            let sx = bar.x + s(14.0);
            // Show process name or fallback to session label
            let label = if !self.process_name.is_empty() {
                &self.process_name
            } else {
                "Sessions"
            };
            // Small green dot to indicate active process
            let dot_sz = s(4.0);
            let dot_y = y_center + ch / 2.0 - dot_sz / 2.0;
            ui.fill(Rect { x: sx, y: dot_y, width: dot_sz, height: dot_sz }, colors::ACCENT_GREEN);
            ui.text(text, label, sx + dot_sz + s(6.0), y_center, colors::FG_MUTED, sidebar_bg);
        }

        // --- Content section: cwd + git branch + dimensions ---
        let content_x = if self.sidebar_width > 0.0 { self.sidebar_width + s(14.0) } else { bar.x + s(14.0) };
        let mut x = content_x;

        // Working directory
        if !self.cwd.is_empty() {
            // Reserve space for right-aligned items
            let hints_label = "? for shortcuts";
            let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
            let right_reserved = text.text_width(&dims) + text.text_width(hints_label) + s(48.0);
            let avail_for_cwd = bar.right() - x - right_reserved - cw * 4.0;
            let max_chars = (avail_for_cwd / cw).floor().max(4.0) as usize;

            let display_cwd = if self.cwd.len() > max_chars {
                format!("\u{2026}{}", &self.cwd[self.cwd.len() - (max_chars - 1)..])
            } else {
                self.cwd.clone()
            };
            ui.text(text, &display_cwd, x, y_center, colors::FG_MUTED, content_bg);
            x += text.text_width(&display_cwd) + cw * 2.0;
        }

        // Git branch with branch icon
        if !self.git_branch.is_empty() {
            let branch_text = format!(" {}", self.git_branch);
            ui.text(text, &branch_text, x, y_center, colors::ACCENT_PEACH, content_bg);
            x += text.text_width(&branch_text) + cw * 2.0;
        }

        // Git diff summary (dynamic)
        if !self.git_diff_summary.is_empty() {
            let file_w = text.text_width(&self.git_diff_summary);
            if x + file_w + cw * 4.0 < bar.right() - s(200.0) {
                ui.text(text, &self.git_diff_summary, x, y_center, colors::FG_MUTED, content_bg);
            }
        }

        // Right-aligned: keyboard hints
        let hints_label = "? for shortcuts";
        let hints_w = text.text_width(hints_label);
        let hints_x = bar.right() - hints_w - s(12.0);
        ui.text(text, hints_label, hints_x, y_center, colors::FG_MUTED, content_bg);

        // Terminal dimensions (left of hints)
        let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
        let dims_width = text.text_width(&dims);
        let dims_x = hints_x - dims_width - s(16.0);
        ui.text(text, &dims, dims_x, y_center, colors::FG_MUTED, content_bg);
    }
}
