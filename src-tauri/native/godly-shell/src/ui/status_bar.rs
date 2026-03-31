//! Status bar: working directory, git branch, terminal dimensions.
//!
//! Each pill (cwd, git branch, dimensions, hints) responds to mouse hover
//! with smooth animated transitions — background brightens, border sharpens,
//! and a subtle lift effect creates tactile feedback.

use super::anim::{self, Anim, lerp, lerp_color};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, MouseEvent};

/// Identifies which status bar pill is hovered (if any).
#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusPill {
    Mode,
    Cwd,
    GitBranch,
    Dims,
    Hints,
}

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
    // Hover state
    hovered_pill: Option<StatusPill>,
    mode_anim: Anim,
    cwd_anim: Anim,
    git_anim: Anim,
    dims_anim: Anim,
    hints_anim: Anim,
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
            hovered_pill: None,
            mode_anim: Anim::default(),
            cwd_anim: Anim::default(),
            git_anim: Anim::default(),
            dims_anim: Anim::default(),
            hints_anim: Anim::default(),
        }
    }

    /// Advance hover animations. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.mode_anim.set(if self.hovered_pill == Some(StatusPill::Mode) { 1.0 } else { 0.0 });
        self.cwd_anim.set(if self.hovered_pill == Some(StatusPill::Cwd) { 1.0 } else { 0.0 });
        self.git_anim.set(if self.hovered_pill == Some(StatusPill::GitBranch) { 1.0 } else { 0.0 });
        self.dims_anim.set(if self.hovered_pill == Some(StatusPill::Dims) { 1.0 } else { 0.0 });
        self.hints_anim.set(if self.hovered_pill == Some(StatusPill::Hints) { 1.0 } else { 0.0 });
        let mut a = false;
        a |= self.mode_anim.tick(hl, dt);
        a |= self.cwd_anim.tick(hl, dt);
        a |= self.git_anim.tick(hl, dt);
        a |= self.dims_anim.tick(hl, dt);
        a |= self.hints_anim.tick(hl, dt);
        a
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer, glow_phase: f32, active_accent: [f32; 4]) {
        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;
        // Proportional UI font advance for text width estimation
        let ui_cw = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };

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

        // Sidebar portion of status bar (subtle gradient for depth)
        if self.sidebar_width > 0.0 {
            let sidebar_status = Rect {
                x: bar.x,
                y: bar.y,
                width: self.sidebar_width,
                height: bar.height,
            };
            let sidebar_bg_bottom = [
                sidebar_bg[0] * 0.92,
                sidebar_bg[1] * 0.92,
                sidebar_bg[2] * 0.92,
                1.0,
            ];
            ui.fill_gradient(sidebar_status, sidebar_bg, sidebar_bg_bottom);
            // Right border on sidebar section — thin line
            ui.vline(self.sidebar_width - 1.0, bar.y, bar.height, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.18]);
            // Very subtle inner shadow for sidebar section depth
            ui.fill_inner_shadow(sidebar_status, [0.0, 0.0, 0.0, 0.02], 0.0, s(3.0));
        }

        // Recessed inner shadow on the content section for depth
        {
            let content_section = Rect {
                x: self.sidebar_width,
                y: bar.y,
                width: bar.width - self.sidebar_width,
                height: bar.height,
            };
            ui.fill_inner_shadow(content_section, [0.0, 0.0, 0.0, 0.02], 0.0, s(2.0));
        }

        // Top separator — thin line, subdued to let color difference do the work.
        ui.hline_fade(bar.x, bar.y, bar.width, 1.0,
            [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.28], s(4.0));
        // Bottom accent stripe — 2px accent-tinted bar at window bottom edge.
        // Three-part gradient (fade-in | solid | fade-out) matching the top
        // accent stripe's treatment for visual symmetry.  Dims when unfocused.
        let breath = 0.92 + 0.08 * glow_phase.sin();
        let bot_accent_alpha = if self.window_focused { 0.20 * breath } else { 0.06 };
        let bot_accent_fade = s(40.0);
        let bot_y = bar.bottom() - 2.0;
        let bot_full = [active_accent[0], active_accent[1], active_accent[2], bot_accent_alpha];
        let bot_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
        let bot_w = bar.width;
        // Fade in from left edge
        ui.fill_gradient_h(
            Rect { x: bar.x, y: bot_y, width: bot_accent_fade, height: 2.0 },
            bot_zero, bot_full,
        );
        // Solid center
        ui.fill(
            Rect { x: bar.x + bot_accent_fade, y: bot_y, width: bot_w - bot_accent_fade * 2.0, height: 2.0 },
            bot_full,
        );
        // Fade out to right edge
        ui.fill_gradient_h(
            Rect { x: bar.x + bot_w - bot_accent_fade, y: bot_y, width: bot_accent_fade, height: 2.0 },
            bot_full, bot_zero,
        );

        let y_center = bar.y + (bar.height - ch) / 2.0;

        // Shared pill rendering helper — creates a hover-responsive pill.
        // `hover_t` drives: background brighten, border sharpen, text lighten.
        let pill_base_top = [
            colors::BG_HOVER[0] * 1.08,
            colors::BG_HOVER[1] * 1.08,
            colors::BG_HOVER[2] * 1.08,
            colors::BG_HOVER[3],
        ];
        let pill_base_bot = colors::BG_HOVER;

        // --- Sidebar section: mode indicator with dot ---
        if self.sidebar_width > 0.0 {
            let sx = bar.x + s(14.0);
            let label_owned;
            let label: &str = if !self.process_name.is_empty() {
                &self.process_name
            } else if !self.connection_status.is_empty() {
                label_owned = self.connection_status.clone();
                &label_owned
            } else {
                "Sessions"
            };
            let pad_h = s(4.0);
            let pad_v = s(2.0);
            let dot_sz = s(4.0);
            let label_w = text.text_width_ui(label);
            let pill_inner_w = dot_sz + s(6.0) + label_w;
            let pill_w = pill_inner_w + pad_h * 2.0;
            let pill_h = ch + pad_v * 2.0;
            let pill_y = bar.y + (bar.height - pill_h) / 2.0;
            let mode_pill_rect = Rect { x: sx, y: pill_y, width: pill_w, height: pill_h };

            let ht = self.mode_anim.value();
            let top = Self::hover_pill_top(pill_base_top, ht, 1.0);
            let bot = Self::hover_pill_bot(pill_base_bot, ht, 1.0);
            let base_border = Self::hover_pill_border(ht, 0.5);
            // Blend a subtle accent tint into the mode pill border for color
            // continuity with the tab bar and sidebar accent language.
            let accent_mix = 0.15;
            let border = [
                base_border[0] * (1.0 - accent_mix) + active_accent[0] * accent_mix,
                base_border[1] * (1.0 - accent_mix) + active_accent[1] * accent_mix,
                base_border[2] * (1.0 - accent_mix) + active_accent[2] * accent_mix,
                base_border[3],
            ];
            // Subtle drop shadow for pill depth
            ui.fill_shadow(
                Rect { x: sx + s(1.0), y: pill_y + s(1.0), width: pill_w - s(2.0), height: pill_h },
                [0.0, 0.0, 0.0, 0.08 + 0.04 * ht], s(3.0), s(3.0),
            );
            ui.fill_rounded_gradient(mode_pill_rect, top, bot, s(3.0));
            ui.stroke_rounded(mode_pill_rect, s(3.0), 0.5, border);
            let dot_y = bar.y + (bar.height - dot_sz) / 2.0;
            let breath = 0.80 + 0.20 * glow_phase.sin();
            let glow_rect = Rect {
                x: sx + pad_h - s(3.0), y: dot_y - s(3.0),
                width: dot_sz + s(6.0), height: dot_sz + s(6.0),
            };
            ui.fill_shadow(glow_rect, [colors::ACCENT_GREEN[0], colors::ACCENT_GREEN[1], colors::ACCENT_GREEN[2], 0.22 * breath], dot_sz, s(5.0));
            ui.fill_rounded(
                Rect { x: sx + pad_h, y: dot_y, width: dot_sz, height: dot_sz },
                colors::ACCENT_GREEN,
                dot_sz / 2.0,
            );
            let label_fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, ht * 0.4);
            ui.text_ui(text, label, sx + pad_h + dot_sz + s(6.0), y_center, label_fg, colors::BG_HOVER);
        }

        // --- Content section: cwd + git branch + dimensions ---
        let content_x = if self.sidebar_width > 0.0 { self.sidebar_width + s(14.0) } else { bar.x + s(14.0) };
        let mut x = content_x;

        // Working directory pill (with folder icon)
        if !self.cwd.is_empty() {
            let hints_label = "? for shortcuts";
            let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
            let right_reserved = text.text_width_ui(&dims) + text.text_width_ui(hints_label)
                + s(4.0) * 4.0
                + s(8.0)
                + s(10.0)
                + ui_cw * 2.0;
            let cwd_pad_h = s(4.0);
            let cwd_pad_v = s(2.0);
            let icon_sz = ch * 0.85;
            let icon_gap = s(4.0);
            let ui_cw = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };
            let avail_for_cwd = bar.right() - x - right_reserved - ui_cw * 4.0 - cwd_pad_h * 2.0 - icon_sz - icon_gap;
            let max_chars = (avail_for_cwd / ui_cw).floor().max(4.0) as usize;

            let display_cwd = if self.cwd.len() > max_chars {
                format!("\u{2026}{}", &self.cwd[self.cwd.len() - (max_chars - 1)..])
            } else {
                self.cwd.clone()
            };
            let cwd_text_w = text.text_width_ui(&display_cwd);
            let cwd_pill_w = icon_sz + icon_gap + cwd_text_w + cwd_pad_h * 2.0;
            let cwd_pill_h = ch + cwd_pad_v * 2.0;
            let cwd_pill_y = bar.y + (bar.height - cwd_pill_h) / 2.0;
            let cwd_pill = Rect { x, y: cwd_pill_y, width: cwd_pill_w, height: cwd_pill_h };

            let ht = self.cwd_anim.value();
            let top = Self::hover_pill_top(pill_base_top, ht, 0.5);
            let bot = Self::hover_pill_bot(pill_base_bot, ht, 0.5);
            let border = Self::hover_pill_border(ht, 0.3);
            // Subtle drop shadow for pill depth
            ui.fill_shadow(
                Rect { x: x + s(1.0), y: cwd_pill_y + s(1.0), width: cwd_pill_w - s(2.0), height: cwd_pill_h },
                [0.0, 0.0, 0.0, 0.06 + 0.04 * ht], s(3.0), s(3.0),
            );
            ui.fill_rounded_gradient(cwd_pill, top, bot, s(3.0));
            ui.stroke_rounded(cwd_pill, s(3.0), 0.5, border);
            // Folder icon
            let icon_y = bar.y + (bar.height - icon_sz) / 2.0;
            let icon_t = (1.0 * text.scale).max(1.0);
            let icon_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, ht * 0.3);
            ui.icon_folder(
                Rect { x: x + cwd_pad_h, y: icon_y, width: icon_sz, height: icon_sz },
                icon_t, icon_fg,
            );
            let cwd_fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, ht * 0.3);
            ui.text_ui(text, &display_cwd, x + cwd_pad_h + icon_sz + icon_gap, y_center, cwd_fg, colors::BG_HOVER);
            x += cwd_pill_w + cw * 2.0;
        }

        // Git branch pill (with branch icon instead of dot)
        if !self.git_branch.is_empty() {
            let pad_h = s(4.0);
            let pad_v = s(2.0);
            let icon_sz = ch * 0.85;
            let icon_gap = s(3.0);
            let branch_w = text.text_width_ui(&self.git_branch);
            let pill_w = icon_sz + icon_gap + branch_w + pad_h * 2.0;
            let pill_h = ch + pad_v * 2.0;
            let pill_y = bar.y + (bar.height - pill_h) / 2.0;
            let git_pill = Rect { x, y: pill_y, width: pill_w, height: pill_h };

            let ht = self.git_anim.value();
            let top = Self::hover_pill_top(pill_base_top, ht, 1.0);
            let bot = Self::hover_pill_bot(pill_base_bot, ht, 1.0);
            let border = Self::hover_pill_border(ht, 0.5);
            // Subtle drop shadow for pill depth
            ui.fill_shadow(
                Rect { x: x + s(1.0), y: pill_y + s(1.0), width: pill_w - s(2.0), height: pill_h },
                [0.0, 0.0, 0.0, 0.08 + 0.04 * ht], s(3.0), s(3.0),
            );
            ui.fill_rounded_gradient(git_pill, top, bot, s(3.0));
            ui.stroke_rounded(git_pill, s(3.0), 0.5, border);
            // Git branch icon (replaces plain dot)
            let icon_y = bar.y + (bar.height - icon_sz) / 2.0;
            let icon_t = (1.0 * text.scale).max(1.0);
            let breath_peach = 0.85 + 0.15 * glow_phase.sin();
            let icon_fg = [
                colors::ACCENT_PEACH[0], colors::ACCENT_PEACH[1],
                colors::ACCENT_PEACH[2], 0.7 + 0.15 * breath_peach,
            ];
            ui.icon_git_branch(
                Rect { x: x + pad_h, y: icon_y, width: icon_sz, height: icon_sz },
                icon_t, icon_fg,
            );
            let branch_fg = lerp_color(colors::ACCENT_PEACH, [1.0, 0.85, 0.72, 1.0], ht * 0.3);
            ui.text_ui(text, &self.git_branch, x + pad_h + icon_sz + icon_gap, y_center, branch_fg, colors::BG_HOVER);
            x += pill_w + cw * 2.0;
        }

        // Git diff summary — styled with colored +/- indicators
        if !self.git_diff_summary.is_empty() {
            let diff_pad_h = s(4.0);
            let diff_pad_v = s(2.0);
            let diff_w = text.text_width_ui(&self.git_diff_summary);
            let diff_pill_w = diff_w + diff_pad_h * 2.0;
            let diff_pill_h = ch + diff_pad_v * 2.0;
            let diff_pill_y = bar.y + (bar.height - diff_pill_h) / 2.0;
            if x + diff_pill_w + cw * 4.0 < bar.right() - s(200.0) {
                let diff_pill = Rect { x, y: diff_pill_y, width: diff_pill_w, height: diff_pill_h };
                // Subtle pill background (no hover animation for this read-only pill)
                let diff_top = [pill_base_top[0], pill_base_top[1], pill_base_top[2], 0.4];
                let diff_bot = [pill_base_bot[0], pill_base_bot[1], pill_base_bot[2], 0.4];
                let diff_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.25];
                ui.fill_rounded_gradient(diff_pill, diff_top, diff_bot, s(3.0));
                ui.stroke_rounded(diff_pill, s(3.0), 0.5, diff_border);
                // Render diff summary with secondary color for readability
                let diff_fg = [colors::FG_SECONDARY[0], colors::FG_SECONDARY[1], colors::FG_SECONDARY[2], 0.8];
                ui.text_ui(text, &self.git_diff_summary, x + diff_pad_h, y_center, diff_fg, colors::BG_HOVER);
                x += diff_pill_w + cw;
            }
        }

        // Right-aligned pills
        let pad_h = s(4.0);
        let pad_v = s(2.0);
        let pill_h = ch + pad_v * 2.0;
        let pill_y = bar.y + (bar.height - pill_h) / 2.0;

        // Keyboard hints pill
        let hints_label = "? for shortcuts";
        let hints_text_w = text.text_width_ui(hints_label);
        let hints_pill_w = hints_text_w + pad_h * 2.0;
        let hints_pill_x = bar.right() - hints_pill_w - s(10.0);
        let hints_rect = Rect { x: hints_pill_x, y: pill_y, width: hints_pill_w, height: pill_h };
        {
            let ht = self.hints_anim.value();
            let top = Self::hover_pill_top(pill_base_top, ht, 1.0);
            let bot = Self::hover_pill_bot(pill_base_bot, ht, 1.0);
            let border = Self::hover_pill_border(ht, 0.5);
            // Subtle drop shadow for pill depth
            ui.fill_shadow(
                Rect { x: hints_pill_x + s(1.0), y: pill_y + s(1.0), width: hints_pill_w - s(2.0), height: pill_h },
                [0.0, 0.0, 0.0, 0.08 + 0.04 * ht], s(3.0), s(3.0),
            );
            ui.fill_rounded_gradient(hints_rect, top, bot, s(3.0));
            ui.stroke_rounded(hints_rect, s(3.0), 0.5, border);
            let fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, ht * 0.3);
            ui.text_ui(text, hints_label, hints_pill_x + pad_h, y_center, fg, colors::BG_HOVER);
        }

        // Metadata labels with vertical divider separators (matches VS Code/Zed)
        let meta_fg = [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.72];
        let divider_fg = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.40];
        let divider_h = ch * 0.6;
        let divider_y = bar.y + (bar.height - divider_h) / 2.0;
        let meta_gap = s(8.0); // gap on each side of a divider

        // Encoding label
        let enc_label = "UTF-8";
        let enc_w = text.text_width_ui(enc_label);
        let enc_x = hints_pill_x - enc_w - s(14.0);
        ui.text_ui(text, enc_label, enc_x, y_center, meta_fg, content_bg);

        // Divider between encoding and line ending
        let div1_x = enc_x - meta_gap;
        ui.vline(div1_x, divider_y, divider_h, 1.0, divider_fg);

        // Line ending label
        let le_label = "LF";
        let le_w = text.text_width_ui(le_label);
        let le_x = div1_x - meta_gap - le_w;
        ui.text_ui(text, le_label, le_x, y_center, meta_fg, content_bg);

        // Divider between line ending and cursor position
        let div2_x = le_x - meta_gap;
        ui.vline(div2_x, divider_y, divider_h, 1.0, divider_fg);

        // Cursor position label — "Ln 1, Col 1" style (matches VS Code/Zed)
        let cursor_label = format!("Ln {}, Col {}", self.cursor_line, self.cursor_col);
        let cursor_w = text.text_width_ui(&cursor_label);
        let cursor_x = div2_x - meta_gap - cursor_w;
        ui.text_ui(text, &cursor_label, cursor_x, y_center, meta_fg, content_bg);

        // Divider between cursor position and dimensions
        let div3_x = cursor_x - meta_gap;
        ui.vline(div3_x, divider_y, divider_h, 1.0, divider_fg);

        // Terminal dimensions pill
        let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
        let dims_text_w = text.text_width_ui(&dims);
        let dims_pill_w = dims_text_w + pad_h * 2.0;
        let dims_pill_x = div3_x - meta_gap - dims_pill_w;
        let dims_rect = Rect { x: dims_pill_x, y: pill_y, width: dims_pill_w, height: pill_h };
        {
            let ht = self.dims_anim.value();
            let top = Self::hover_pill_top(pill_base_top, ht, 1.0);
            let bot = Self::hover_pill_bot(pill_base_bot, ht, 1.0);
            let border = Self::hover_pill_border(ht, 0.5);
            // Subtle drop shadow for pill depth
            ui.fill_shadow(
                Rect { x: dims_pill_x + s(1.0), y: pill_y + s(1.0), width: dims_pill_w - s(2.0), height: pill_h },
                [0.0, 0.0, 0.0, 0.08 + 0.04 * ht], s(3.0), s(3.0),
            );
            ui.fill_rounded_gradient(dims_rect, top, bot, s(3.0));
            ui.stroke_rounded(dims_rect, s(3.0), 0.5, border);
            let fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, ht * 0.3);
            ui.text_ui(text, &dims, dims_pill_x + pad_h, y_center, fg, colors::BG_HOVER);
        }
    }

    // -- Pill hover color helpers -------------------------------------------
    // These produce smooth transitions between rest and hovered pill states.
    // `base_alpha` is the rest-state alpha (1.0 for fully opaque pills, 0.5 for
    // semi-transparent ones like the cwd pill).

    /// Hovered pill top color: brightens toward BG_SURFACE for a visible "lift".
    /// On dark themes, multiplicative boosts are invisible — we need additive shift.
    fn hover_pill_top(base_top: [f32; 4], hover_t: f32, base_alpha: f32) -> [f32; 4] {
        // Additive brighten: blend toward a lighter surface color
        let target = colors::BG_HOVER;
        let alpha = lerp(base_alpha, 1.0, hover_t);
        [
            lerp(base_top[0], target[0] * 1.4, hover_t),
            lerp(base_top[1], target[1] * 1.4, hover_t),
            lerp(base_top[2], target[2] * 1.4, hover_t),
            base_top[3] * alpha,
        ]
    }

    /// Hovered pill bottom color: slightly less bright than top for gradient.
    fn hover_pill_bot(base_bot: [f32; 4], hover_t: f32, base_alpha: f32) -> [f32; 4] {
        let target = colors::BG_HOVER;
        let alpha = lerp(base_alpha, 1.0, hover_t);
        [
            lerp(base_bot[0], target[0] * 1.2, hover_t),
            lerp(base_bot[1], target[1] * 1.2, hover_t),
            lerp(base_bot[2], target[2] * 1.2, hover_t),
            base_bot[3] * alpha,
        ]
    }

    fn hover_pill_border(hover_t: f32, base_alpha: f32) -> [f32; 4] {
        [
            lerp(colors::BORDER[0], colors::BORDER[0] * 1.5, hover_t),
            lerp(colors::BORDER[1], colors::BORDER[1] * 1.5, hover_t),
            lerp(colors::BORDER[2], colors::BORDER[2] * 1.5, hover_t),
            lerp(base_alpha, 1.0, hover_t),
        ]
    }

    /// Compute pill rectangles for hit-testing. Must stay in sync with `build()`.
    fn pill_rects(&self, bar: Rect, text: &UiTextRenderer) -> Vec<(Rect, StatusPill)> {
        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;
        let pad_h = s(4.0);
        let pad_v = s(2.0);
        let pill_h = ch + pad_v * 2.0;
        let pill_y = bar.y + (bar.height - pill_h) / 2.0;
        let mut pills = Vec::new();

        // Mode pill (sidebar section)
        if self.sidebar_width > 0.0 {
            let sx = bar.x + s(14.0);
            let label = if !self.process_name.is_empty() { &self.process_name } else { "Sessions" };
            let dot_sz = s(4.0);
            let label_w = text.text_width_ui(label);
            let pill_w = dot_sz + s(6.0) + label_w + pad_h * 2.0;
            pills.push((Rect { x: sx, y: pill_y, width: pill_w, height: pill_h }, StatusPill::Mode));
        }

        // Content pills
        let content_x = if self.sidebar_width > 0.0 { self.sidebar_width + s(14.0) } else { bar.x + s(14.0) };
        let mut x = content_x;

        // Cwd pill
        if !self.cwd.is_empty() {
            let hints_label = "? for shortcuts";
            let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
            let ui_cw2 = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };
            let right_reserved = text.text_width_ui(&dims) + text.text_width_ui(hints_label)
                + s(4.0) * 4.0 + s(8.0) + s(10.0) + ui_cw2 * 2.0;
            let avail = bar.right() - x - right_reserved - ui_cw2 * 4.0 - pad_h * 2.0;
            let max_chars = (avail / ui_cw2).floor().max(4.0) as usize;
            let display_cwd = if self.cwd.len() > max_chars {
                format!("\u{2026}{}", &self.cwd[self.cwd.len() - (max_chars - 1)..])
            } else {
                self.cwd.clone()
            };
            let cwd_text_w = text.text_width_ui(&display_cwd);
            let cwd_pill_w = cwd_text_w + pad_h * 2.0;
            let cwd_pill_h = ch + pad_v * 2.0;
            let cwd_pill_y = bar.y + (bar.height - cwd_pill_h) / 2.0;
            pills.push((Rect { x, y: cwd_pill_y, width: cwd_pill_w, height: cwd_pill_h }, StatusPill::Cwd));
            x += cwd_pill_w + cw * 2.0;
        }

        // Git branch pill
        if !self.git_branch.is_empty() {
            let dot_sz = s(4.0);
            let branch_text = format!(" {}", self.git_branch);
            let branch_w = text.text_width_ui(&branch_text);
            let pill_w = dot_sz + s(4.0) + branch_w + pad_h * 2.0;
            pills.push((Rect { x, y: pill_y, width: pill_w, height: pill_h }, StatusPill::GitBranch));
        }

        // Right-aligned pills
        let hints_label = "? for shortcuts";
        let hints_text_w = text.text_width_ui(hints_label);
        let hints_pill_w = hints_text_w + pad_h * 2.0;
        let hints_pill_x = bar.right() - hints_pill_w - s(10.0);
        pills.push((Rect { x: hints_pill_x, y: pill_y, width: hints_pill_w, height: pill_h }, StatusPill::Hints));

        let dims = format!("{}x{}", self.terminal_size.1, self.terminal_size.0);
        let dims_text_w = text.text_width_ui(&dims);
        let dims_pill_w = dims_text_w + pad_h * 2.0;
        let dims_pill_x = hints_pill_x - dims_pill_w - s(8.0);
        pills.push((Rect { x: dims_pill_x, y: pill_y, width: dims_pill_w, height: pill_h }, StatusPill::Dims));

        pills
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect, text: &UiTextRenderer) {
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_pill = None;
                if bar.contains(x, y) {
                    for (rect, pill) in self.pill_rects(bar, text) {
                        if rect.contains(x, y) {
                            self.hovered_pill = Some(pill);
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
