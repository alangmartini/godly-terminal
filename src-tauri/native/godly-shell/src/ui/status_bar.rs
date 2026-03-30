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

        // Background — subtle gradient for depth (raised → slightly darker at bottom)
        let sidebar_bg = colors::BG_DARK;
        let content_bg = colors::BG_SURFACE;
        let content_bg_bottom = [
            content_bg[0] * 0.92,
            content_bg[1] * 0.92,
            content_bg[2] * 0.92,
            1.0,
        ];
        ui.fill_gradient(bar, content_bg, content_bg_bottom);

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

        // Top separator line + inner bevel highlight (consistent with tab bar)
        ui.hline(bar.x, bar.y, bar.width, 1.0, colors::BORDER);
        ui.hline(bar.x, bar.y + 1.0, bar.width, 1.0, [1.0, 1.0, 1.0, 0.025]);

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
            // Pill padding constants
            let pad_h = s(4.0);
            let pad_v = s(2.0);
            // Small green dot (circle via SDF) to indicate active process
            let dot_sz = s(4.0);
            let label_w = text.text_width(label);
            // Pill covers: left-pad + dot + gap + label + right-pad
            let pill_inner_w = dot_sz + s(6.0) + label_w;
            let pill_w = pill_inner_w + pad_h * 2.0;
            let pill_h = ch + pad_v * 2.0;
            let pill_y = bar.y + (bar.height - pill_h) / 2.0;
            let pill_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5];
            let mode_pill_top = [
                colors::BG_HOVER[0] * 1.08,
                colors::BG_HOVER[1] * 1.08,
                colors::BG_HOVER[2] * 1.08,
                colors::BG_HOVER[3],
            ];
            let mode_pill_rect = Rect { x: sx, y: pill_y, width: pill_w, height: pill_h };
            ui.fill_rounded_gradient(mode_pill_rect, mode_pill_top, colors::BG_HOVER, s(3.0));
            ui.stroke_rounded(mode_pill_rect, s(3.0), 0.5, pill_border);
            let dot_y = bar.y + (bar.height - dot_sz) / 2.0;
            ui.fill_rounded(
                Rect { x: sx + pad_h, y: dot_y, width: dot_sz, height: dot_sz },
                colors::ACCENT_GREEN,
                dot_sz / 2.0,
            );
            ui.text(text, label, sx + pad_h + dot_sz + s(6.0), y_center, colors::FG_SECONDARY, colors::BG_HOVER);
        }

        // --- Content section: cwd + git branch + dimensions ---
        let content_x = if self.sidebar_width > 0.0 { self.sidebar_width + s(14.0) } else { bar.x + s(14.0) };
        let mut x = content_x;

        // Working directory
        if !self.cwd.is_empty() {
            // Reserve space for right-aligned pill items:
            //   hints pill: text + 2*pad_h + outer margin
            //   dims pill:  text + 2*pad_h + gap between pills
            let hints_label = "? for shortcuts";
            let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
            let right_reserved = text.text_width(&dims) + text.text_width(hints_label)
                + s(4.0) * 4.0   // 2x pad_h per pill
                + s(8.0)          // gap between the two pills
                + s(10.0)         // outer right margin
                + cw * 2.0;       // breathing room
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

        // Git branch with branch icon wrapped in a gradient pill
        if !self.git_branch.is_empty() {
            let pad_h = s(4.0);
            let pad_v = s(2.0);
            // Small dot before branch name
            let dot_sz = s(4.0);
            let branch_text = format!(" {}", self.git_branch);
            let branch_w = text.text_width(&branch_text);
            let pill_inner_w = dot_sz + s(4.0) + branch_w;
            let pill_w = pill_inner_w + pad_h * 2.0;
            let pill_h = ch + pad_v * 2.0;
            let pill_y = bar.y + (bar.height - pill_h) / 2.0;
            let pill_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5];
            // Gradient pill: slightly lighter top for 3D depth
            let pill_top = [
                colors::BG_HOVER[0] * 1.08,
                colors::BG_HOVER[1] * 1.08,
                colors::BG_HOVER[2] * 1.08,
                colors::BG_HOVER[3],
            ];
            ui.fill_rounded_gradient(
                Rect { x, y: pill_y, width: pill_w, height: pill_h },
                pill_top, colors::BG_HOVER,
                s(3.0),
            );
            // Pill border overlay
            ui.stroke_rounded(
                Rect { x, y: pill_y, width: pill_w, height: pill_h },
                s(3.0), 0.5, pill_border,
            );
            let dot_y = bar.y + (bar.height - dot_sz) / 2.0;
            ui.fill_rounded(
                Rect { x: x + pad_h, y: dot_y, width: dot_sz, height: dot_sz },
                colors::ACCENT_PEACH,
                dot_sz / 2.0,
            );
            ui.text(text, &branch_text, x + pad_h + dot_sz + s(4.0) - cw, y_center, colors::ACCENT_PEACH, colors::BG_HOVER);
            x += pill_w + cw * 2.0;
        }

        // Git diff summary (dynamic)
        if !self.git_diff_summary.is_empty() {
            let file_w = text.text_width(&self.git_diff_summary);
            if x + file_w + cw * 4.0 < bar.right() - s(200.0) {
                ui.text(text, &self.git_diff_summary, x, y_center, colors::FG_MUTED, content_bg);
            }
        }

        // Right-aligned: keyboard hints pill (gradient for 3D depth)
        let pad_h = s(4.0);
        let pad_v = s(2.0);
        let pill_h = ch + pad_v * 2.0;
        let pill_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5];
        let pill_top = [
            colors::BG_HOVER[0] * 1.08,
            colors::BG_HOVER[1] * 1.08,
            colors::BG_HOVER[2] * 1.08,
            colors::BG_HOVER[3],
        ];
        let hints_label = "? for shortcuts";
        let hints_text_w = text.text_width(hints_label);
        let hints_pill_w = hints_text_w + pad_h * 2.0;
        let hints_pill_x = bar.right() - hints_pill_w - s(10.0);
        let pill_y = bar.y + (bar.height - pill_h) / 2.0;
        let hints_rect = Rect { x: hints_pill_x, y: pill_y, width: hints_pill_w, height: pill_h };
        ui.fill_rounded_gradient(hints_rect, pill_top, colors::BG_HOVER, s(3.0));
        ui.stroke_rounded(hints_rect, s(3.0), 0.5, pill_border);
        ui.text(text, hints_label, hints_pill_x + pad_h, y_center, colors::FG_MUTED, colors::BG_HOVER);

        // Terminal dimensions pill (left of hints, gradient)
        let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
        let dims_text_w = text.text_width(&dims);
        let dims_pill_w = dims_text_w + pad_h * 2.0;
        let dims_pill_x = hints_pill_x - dims_pill_w - s(8.0);
        let dims_rect = Rect { x: dims_pill_x, y: pill_y, width: dims_pill_w, height: pill_h };
        ui.fill_rounded_gradient(dims_rect, pill_top, colors::BG_HOVER, s(3.0));
        ui.stroke_rounded(dims_rect, s(3.0), 0.5, pill_border);
        ui.text(text, &dims, dims_pill_x + pad_h, y_center, colors::FG_MUTED, colors::BG_HOVER);
    }
}
