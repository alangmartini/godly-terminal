//! Status bar: flat text layout matching the web reference.
//!
//! Left side: streaming state indicator or process name.
//! Right side: working directory, git branch (amber), git diff stats.

use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::widget::Rect;

pub struct StatusBar {
    pub process_name: String,
    pub cwd: String,
    pub git_branch: String,
    pub terminal_size: (u16, u16),
    pub sidebar_width: f32,
    pub git_diff_summary: String,
    /// Connection status label (e.g. "Ready", "Connecting...", "Disconnected").
    pub connection_status: String,
    /// Cursor line position (1-indexed).
    pub cursor_line: u32,
    /// Cursor column position (1-indexed).
    pub cursor_col: u32,
    /// Whether the parent window has input focus (dims accents when false).
    pub window_focused: bool,
    /// Whether the terminal is actively streaming output.
    pub streaming: bool,
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
            connection_status: "Ready".into(),
            cursor_line: 1,
            cursor_col: 1,
            window_focused: true,
            streaming: false,
        }
    }

    /// No hover animations needed — returns false.
    pub fn tick_animations(&mut self, _dt: f32) -> bool {
        false
    }

    pub fn build(
        &self,
        ui: &mut UiBuilder,
        bar: Rect,
        text: &UiTextRenderer,
        glow_phase: f32,
        _active_accent: [f32; 4],
    ) {
        let s = |v: f32| text.s(v);
        let ch = text.cell_height;

        // Flat background — BG_STATUS
        ui.fill(bar, colors::BG_STATUS);

        // Sidebar portion (slightly different shade)
        if self.sidebar_width > 0.0 {
            let sidebar_status = Rect {
                x: bar.x,
                y: bar.y,
                width: self.sidebar_width,
                height: bar.height,
            };
            ui.fill(sidebar_status, colors::BG_DARK);
            // Right border on sidebar section — solid, matching other separators
            ui.vline(
                self.sidebar_width - 1.0,
                bar.y,
                bar.height,
                1.0,
                colors::BORDER,
            );
        }

        // Top separator — solid 1px hairline matching web reference
        // (borderTop: "1px solid #1a1d25").
        ui.hline(bar.x, bar.y, bar.width, 1.0, colors::BORDER);

        // Web: fontSize 11 for all status bar text
        let sc = font_scale::PX11;
        let sch = ch * sc;
        let y_center = bar.y + (bar.height - sch) / 2.0;
        let bg = colors::BG_STATUS;

        // --- Left side: streaming state or process name ---
        let left_x = if self.sidebar_width > 0.0 {
            self.sidebar_width + s(14.0)
        } else {
            bar.x + s(14.0)
        };
        let mut x = left_x;

        if self.streaming {
            // Pulsing ~ indicator — web: fontSize 9, animation pulse 1.5s
            let pulse = 0.8 - 0.2 * (glow_phase * 3.5 / 1.5).cos(); // 1.5s period
            let tilde_fg = [
                colors::FG_SECONDARY[0],
                colors::FG_SECONDARY[1],
                colors::FG_SECONDARY[2],
                pulse,
            ];
            let tilde_sc = font_scale::PX9; // web: fontSize 9 (smaller than surrounding text)
            let tilde_ch = ch * tilde_sc;
            let tilde_y = bar.y + (bar.height - tilde_ch) / 2.0;
            ui.text_ui_scaled(text, "~", x, tilde_y, tilde_fg, bg, tilde_sc);
            x += text.text_width_ui_scaled("~", tilde_sc) + s(5.0); // web: gap 5

            ui.text_ui_scaled(
                text,
                "Streaming response\u{2026}",
                x,
                y_center,
                colors::FG_SECONDARY,
                bg,
                sc,
            );
            x += text.text_width_ui_scaled("Streaming response\u{2026}", sc) + s(12.0);

            let esc_fg = colors::FG_MUTED;
            ui.text_ui_scaled(text, "Esc to cancel", x, y_center, esc_fg, bg, sc);
        } else if !self.process_name.is_empty() {
            ui.text_ui_scaled(
                text,
                &self.process_name,
                x,
                y_center,
                colors::FG_SECONDARY,
                bg,
                sc,
            );
        }

        // --- Right side: path | branch | git diff ---
        // Web reference uses much darker text here than the main UI:
        //   path: #3b4048, separators: #2d333b, diff text: #484f58
        let mut rx = bar.right() - s(14.0);
        let gap = s(6.0); // web: flex container gap: 6
        let sep_w = text.text_width_ui_scaled("|", sc);
        let sep_fg = colors::BG_HOVER; // web: #2d333b

        // Git diff stats (rightmost)
        if !self.git_diff_summary.is_empty() {
            // Parse simple diff format like "+21 ~4 -70" and colorize.
            // Web renders each token as a separate <span> child with gap: 6.
            let tokens: Vec<&str> = self.git_diff_summary.split_whitespace().collect();
            // Measure total width: sum of token widths + gap between each token
            let total_w: f32 = tokens.iter().map(|t| text.text_width_ui_scaled(t, sc)).sum::<f32>()
                + gap * (tokens.len().saturating_sub(1)) as f32;
            rx -= total_w;
            let mut dx = rx;
            for (i, token) in tokens.iter().enumerate() {
                let color = if token.starts_with('+') {
                    colors::ACCENT_GREEN
                } else if token.starts_with('-') || token.starts_with('~') {
                    colors::ACCENT_RED // web: ~N (modifications) also red
                } else {
                    colors::STATUS_DEFAULT // web: #484f58 (inherited status bar color)
                };
                ui.text_ui_scaled(text, token, dx, y_center, color, bg, sc);
                dx += text.text_width_ui_scaled(token, sc);
                if i + 1 < tokens.len() {
                    dx += gap;
                }
            }

            // Separator: gap + "|" + gap
            rx -= gap + sep_w + gap;
            ui.text_ui_scaled(text, "|", rx + gap, y_center, sep_fg, bg, sc);
        }

        // Git branch (amber)
        if !self.git_branch.is_empty() {
            let branch_display = format!("({})", self.git_branch);
            let branch_w = text.text_width_ui_scaled(&branch_display, sc);
            rx -= branch_w;
            ui.text_ui_scaled(
                text,
                &branch_display,
                rx,
                y_center,
                colors::ACCENT_PEACH,
                bg,
                sc,
            );

            // Separator: gap + "|" + gap
            rx -= gap + sep_w + gap;
            ui.text_ui_scaled(text, "|", rx + gap, y_center, sep_fg, bg, sc);
        }

        // Working directory path (muted)
        if !self.cwd.is_empty() {
            // Truncate from the left if too long
            let ui_cw = if text.ui_avg_advance > 0.0 {
                text.ui_avg_advance * sc
            } else {
                text.cell_width * 0.75 * sc
            };
            let avail = rx - left_x - s(40.0);
            let max_chars = (avail / ui_cw).floor().max(8.0) as usize;
            let display = if self.cwd.len() > max_chars {
                format!("\u{2026}{}", &self.cwd[self.cwd.len() - (max_chars - 1)..])
            } else {
                self.cwd.clone()
            };
            let path_w = text.text_width_ui_scaled(&display, sc);
            rx -= path_w;
            ui.text_ui_scaled(text, &display, rx, y_center, colors::STATUS_PATH, bg, sc);
            // web: #3b4048
        }
    }

    pub fn on_mouse(
        &mut self,
        _event: super::widget::MouseEvent,
        _bar: Rect,
        _text: &UiTextRenderer,
    ) {
        // No interactive pills — mouse events are no-ops
    }
}
