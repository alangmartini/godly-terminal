//! Tab bar: horizontal row of tab buttons with numbered indicators.
//! Also serves as the title bar (window drag + min/max/close buttons).

use super::anim::{self, Anim, AnimVec, lerp_color, lerp};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const TAB_MAX_WIDTH: f32 = 170.0;
const TAB_MIN_WIDTH: f32 = 90.0;
const TAB_GAP: f32 = 1.0;
const TAB_MARGIN_LEFT: f32 = 6.0;
const TAB_INSET_V: f32 = 3.0;
const BUTTON_WIDTH: f32 = 46.0;
const ICON_LINE_T: f32 = 1.2;

/// Accent colors for each tab position (cycles if more tabs).
const TAB_ACCENTS: &[[f32; 4]] = &[
    colors::ACCENT_BLUE,
    colors::ACCENT_GREEN,
    colors::ACCENT_PEACH,
    colors::ACCENT_MAUVE,
    colors::ACCENT_RED,
];

pub struct TabInfo {
    pub id: String,
    pub title: String,
    pub active: bool,
    /// Number of unread lines of output since this tab was last active.
    /// Rendered as a small badge on inactive tabs.
    pub unread_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

pub struct TabBar {
    pub tabs: Vec<TabInfo>,
    pub hovered_tab: Option<usize>,
    pub hovered_close_tab: Option<usize>,
    pub hovered_button: Option<WindowButton>,
    pub hovered_new_tab: bool,
    pub sidebar_width: f32,
    // Smooth animation state
    tab_hover_anim: AnimVec,
    close_hover_anim: AnimVec,
    new_tab_anim: Anim,
    btn_minimize_anim: Anim,
    btn_maximize_anim: Anim,
    btn_close_anim: Anim,
    /// Smooth transition when tabs switch between active/inactive states.
    active_anim: AnimVec,
    /// Continuous phase for ambient breathing glow on active tab accent.
    glow_phase: f32,
}

impl TabBar {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            hovered_tab: None,
            hovered_close_tab: None,
            hovered_button: None,
            hovered_new_tab: false,
            sidebar_width: 0.0,
            tab_hover_anim: AnimVec::default(),
            close_hover_anim: AnimVec::default(),
            new_tab_anim: Anim::default(),
            btn_minimize_anim: Anim::default(),
            btn_maximize_anim: Anim::default(),
            btn_close_anim: Anim::default(),
            active_anim: AnimVec::default(),
            glow_phase: 0.0,
        }
    }

    /// Advance all hover animations. `dt` = seconds since last frame. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        // Update targets from current hover state
        self.tab_hover_anim.ensure_len(self.tabs.len());
        self.close_hover_anim.ensure_len(self.tabs.len());
        for i in 0..self.tabs.len() {
            self.tab_hover_anim.set(i, if self.hovered_tab == Some(i) { 1.0 } else { 0.0 });
            self.close_hover_anim.set(i, if self.hovered_close_tab == Some(i) { 1.0 } else { 0.0 });
        }
        self.active_anim.ensure_len(self.tabs.len());
        for i in 0..self.tabs.len() {
            self.active_anim.set(i, if self.tabs[i].active { 1.0 } else { 0.0 });
        }
        self.new_tab_anim.set(if self.hovered_new_tab { 1.0 } else { 0.0 });
        self.btn_minimize_anim.set(if self.hovered_button == Some(WindowButton::Minimize) { 1.0 } else { 0.0 });
        self.btn_maximize_anim.set(if self.hovered_button == Some(WindowButton::Maximize) { 1.0 } else { 0.0 });
        self.btn_close_anim.set(if self.hovered_button == Some(WindowButton::Close) { 1.0 } else { 0.0 });

        let mut animating = false;
        animating |= self.tab_hover_anim.tick(hl, dt);
        animating |= self.close_hover_anim.tick(hl, dt);
        animating |= self.active_anim.tick(anim::timing::HOVER, dt);
        animating |= self.new_tab_anim.tick(hl, dt);
        animating |= self.btn_minimize_anim.tick(hl, dt);
        animating |= self.btn_maximize_anim.tick(hl, dt);
        animating |= self.btn_close_anim.tick(hl, dt);

        // Breathing glow on active tab accent (~3.5s cycle, frame-rate independent)
        if self.tabs.iter().any(|t| t.active) {
            self.glow_phase += dt * std::f32::consts::TAU / 3.5;
            if self.glow_phase > std::f32::consts::TAU { self.glow_phase -= std::f32::consts::TAU; }
            animating = true;
        }

        animating
    }

    /// The x-offset where tabs begin (after sidebar area).
    fn tabs_origin_x(&self, bar: Rect, scale: f32) -> f32 {
        let tab_margin = (TAB_MARGIN_LEFT * scale).round();
        bar.x + self.sidebar_width + tab_margin
    }

    /// Compute the effective tab width that fits all tabs in the available space.
    fn effective_tab_width(&self, bar: Rect, scale: f32) -> f32 {
        let tab_gap = (TAB_GAP * scale).round();
        let btn_reserve = (BUTTON_WIDTH * scale).round() * 3.0;
        let n = self.tabs.len().max(1) as f32;
        let origin = self.tabs_origin_x(bar, scale);
        // new-tab button + gap before window controls
        let new_tab_reserve = (36.0 * scale).round();
        let sep_reserve = (12.0 * scale).round();
        let avail = bar.right() - origin - btn_reserve - new_tab_reserve - sep_reserve;
        let per_tab = ((avail - (n - 1.0) * tab_gap) / n).floor();
        per_tab.clamp((TAB_MIN_WIDTH * scale).round(), (TAB_MAX_WIDTH * scale).round())
    }

    fn tab_rect(&self, index: usize, bar: Rect, scale: f32) -> Rect {
        let tab_w = self.effective_tab_width(bar, scale);
        let tab_gap = (TAB_GAP * scale).round();
        let tab_inset = (TAB_INSET_V * scale).round();
        let origin = self.tabs_origin_x(bar, scale);
        Rect {
            x: origin + index as f32 * (tab_w + tab_gap),
            y: bar.y + tab_inset,
            width: tab_w,
            height: bar.height - tab_inset,
        }
    }

    fn window_button_rects(&self, bar: Rect, scale: f32) -> [(Rect, WindowButton); 3] {
        let btn_w = (BUTTON_WIDTH * scale).round();
        let close = Rect {
            x: bar.right() - btn_w,
            y: bar.y,
            width: btn_w,
            height: bar.height,
        };
        let maximize = Rect {
            x: close.x - btn_w,
            y: bar.y,
            width: btn_w,
            height: bar.height,
        };
        let minimize = Rect {
            x: maximize.x - btn_w,
            y: bar.y,
            width: btn_w,
            height: bar.height,
        };
        [
            (minimize, WindowButton::Minimize),
            (maximize, WindowButton::Maximize),
            (close, WindowButton::Close),
        ]
    }

    fn new_tab_rect(&self, bar: Rect, scale: f32) -> Rect {
        let tab_w = self.effective_tab_width(bar, scale);
        let tab_gap = (TAB_GAP * scale).round();
        let origin = self.tabs_origin_x(bar, scale);
        let new_x = origin + self.tabs.len() as f32 * (tab_w + tab_gap) + (8.0 * scale).round();
        let btn_sz = (24.0 * scale).round();
        Rect {
            x: new_x,
            y: bar.y + (bar.height - btn_sz) / 2.0,
            width: btn_sz,
            height: btn_sz,
        }
    }

    /// Current breathing glow phase (for sharing with other components).
    pub fn glow_phase(&self) -> f32 { self.glow_phase }

    fn accent_for(&self, index: usize) -> [f32; 4] {
        TAB_ACCENTS[index % TAB_ACCENTS.len()]
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;
        let text_y = |area_h: f32, y: f32| y + (area_h - ch) / 2.0;

        let tab_w = self.effective_tab_width(bar, text.scale);
        let tab_gap = s(TAB_GAP);
        let tab_inset = s(TAB_INSET_V);
        let origin = self.tabs_origin_x(bar, text.scale);

        // Background — subtle top-to-bottom gradient for depth
        let bar_top = colors::BG_DARK;
        let bar_bottom = [
            colors::BG_DARK[0] * 0.92,
            colors::BG_DARK[1] * 0.92,
            colors::BG_DARK[2] * 0.92,
            1.0,
        ];
        ui.fill_gradient(bar, bar_top, bar_bottom);

        // Window top accent stripe — 2px line at the very top of the window
        // using the active tab's accent color. Professional brand touch found
        // in VS Code, JetBrains, and modern editors.
        let active_accent = self.tabs.iter().enumerate()
            .find(|(_, t)| t.active)
            .map(|(i, _)| self.accent_for(i))
            .unwrap_or(colors::ACCENT_BLUE);
        {
            let stripe_h = s(2.0);
            let stripe_rect = Rect {
                x: bar.x,
                y: bar.y,
                width: bar.width,
                height: stripe_h,
            };
            ui.fill(stripe_rect, active_accent);
            // Subtle glow spill below the stripe for depth
            let breath = 0.92 + 0.08 * self.glow_phase.sin();
            let glow_rect = Rect {
                x: bar.x,
                y: bar.y + stripe_h,
                width: bar.width,
                height: s(4.0),
            };
            ui.fill_gradient(glow_rect,
                [active_accent[0], active_accent[1], active_accent[2], 0.10 * breath],
                [active_accent[0], active_accent[1], active_accent[2], 0.0],
            );
        }

        // (Glass sheen and bevel removed — clean flat bar matches Zed/VS Code restraint)

        // Bottom separator line — break under the active tab for seamless join
        let active_rect = self.tabs.iter().enumerate()
            .find(|(_, t)| t.active)
            .map(|(i, _)| self.tab_rect(i, bar, text.scale));
        if let Some(ar) = active_rect {
            let ear_r = s(5.0); // radius of the inverse corner "ear" shapes
            // Left segment (bar start to active tab left edge, shortened for ear)
            if ar.x > bar.x {
                let seg_end = ar.x - ear_r;
                if seg_end > bar.x {
                    ui.hline_aa(bar.x, bar.bottom() - 1.0, seg_end - bar.x, 1.0,
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5]);
                }
                // Left ear: small rounded corner that creates concave curve
                // Only bottom-right corner is rounded to form the inverse curve
                // Uses bar_bottom color (not BG_DARK) to match gradient at this y-position
                let ear_rect = Rect {
                    x: ar.x - ear_r,
                    y: bar.bottom() - ear_r,
                    width: ear_r,
                    height: ear_r,
                };
                ui.fill_rounded_custom(ear_rect, bar_bottom, [0.0, 0.0, ear_r, 0.0]);
            }
            // Right segment (active tab right edge to bar end, shortened for ear)
            if ar.right() < bar.right() {
                let seg_start = ar.right() + ear_r;
                if seg_start < bar.right() {
                    ui.hline_aa(seg_start, bar.bottom() - 1.0, bar.right() - seg_start, 1.0,
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5]);
                }
                // Right ear: concave curve on the other side
                // Uses bar_bottom color to match gradient at this y-position
                let ear_rect = Rect {
                    x: ar.right(),
                    y: bar.bottom() - ear_r,
                    width: ear_r,
                    height: ear_r,
                };
                ui.fill_rounded_custom(ear_rect, bar_bottom, [0.0, 0.0, 0.0, ear_r]);
            }
            // Accent glow bleed below active tab — breathing Gaussian emission
            // Creates a warm light-spill effect from the active tab into content
            let active_accent = self.tabs.iter().enumerate()
                .find(|(_, t)| t.active)
                .map(|(i, _)| self.accent_for(i))
                .unwrap_or(colors::ACCENT_BLUE);
            let breath = 0.92 + 0.08 * self.glow_phase.sin();
            let glow_rect = Rect {
                x: ar.x + s(6.0),
                y: bar.bottom() - s(1.0),
                width: ar.width - s(12.0),
                height: s(3.0),
            };
            ui.fill_shadow(glow_rect,
                [active_accent[0], active_accent[1], active_accent[2], 0.06 * breath],
                s(2.0), s(5.0),
            );
        } else {
            ui.hline_aa(bar.x, bar.bottom() - 1.0, bar.width, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5]);
        }

        // Sidebar section: "Godly Terminal" branding with subtle differentiation
        if self.sidebar_width > 0.0 {
            // Slightly darker background for branding section (matches sidebar tone)
            let brand_bg_top = [
                colors::BG_DARK[0] * 1.02,
                colors::BG_DARK[1] * 1.02,
                colors::BG_DARK[2] * 1.02,
                1.0,
            ];
            let brand_bg_bot = [
                colors::BG_DARK[0] * 0.96,
                colors::BG_DARK[1] * 0.96,
                colors::BG_DARK[2] * 0.96,
                1.0,
            ];
            let brand_section = Rect {
                x: bar.x, y: bar.y,
                width: self.sidebar_width, height: bar.height,
            };
            ui.fill_gradient(brand_section, brand_bg_top, brand_bg_bot);
            let icon_size = ch * 1.1;
            let icon_x = bar.x + s(10.0);
            let icon_y = bar.y + (bar.height - icon_size) / 2.0;
            let brand_x = icon_x + icon_size + s(6.0);
            let brand_y = text_y(bar.height, bar.y);
            // Branding text with subtle accent tint from active tab color
            let active_accent = self.tabs.iter().enumerate()
                .find(|(_, t)| t.active)
                .map(|(i, _)| self.accent_for(i))
                .unwrap_or(colors::ACCENT_BLUE);
            let brand_fg = [
                colors::FG_SECONDARY[0] * 0.85 + active_accent[0] * 0.15,
                colors::FG_SECONDARY[1] * 0.85 + active_accent[1] * 0.15,
                colors::FG_SECONDARY[2] * 0.85 + active_accent[2] * 0.15,
                colors::FG_SECONDARY[3],
            ];
            // Terminal icon with accent tint
            let icon_fg = [
                colors::FG_MUTED[0] * 0.7 + active_accent[0] * 0.3,
                colors::FG_MUTED[1] * 0.7 + active_accent[1] * 0.3,
                colors::FG_MUTED[2] * 0.7 + active_accent[2] * 0.3,
                0.6,
            ];
            let icon_t = (1.2 * text.scale).max(1.0);
            ui.icon_terminal(
                Rect { x: icon_x, y: icon_y, width: icon_size, height: icon_size },
                icon_t, icon_fg,
            );
            ui.text_ui_bold(text, "Godly Terminal", brand_x, brand_y,
                    brand_fg, colors::BG_DARK);
            // Right border for sidebar section — thin line
            ui.vline(self.sidebar_width - 1.0, bar.y + s(6.0), bar.height - s(12.0), 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.18]);
        }

        // Icon line thickness (used for close buttons in tabs and window controls)
        let icon_t = (ICON_LINE_T * text.scale).max(1.0);

        // Close button dimensions (reserved on right side of every tab)
        let close_btn_sz = s(16.0);
        let close_btn_pad = s(8.0);

        // Proportional UI font advance for tab title width estimation.
        // Using ui_avg_advance gives tighter truncation than monospace cell_width
        // because proportional characters are narrower on average (~75% of cell_width).
        let ui_cw = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };

        // Max chars for tab title (dynamic based on effective width, reserves close button space)
        // Badge is ch*0.9 wide circle at x+10, then gap before title
        let badge_sz = ch * 0.9;
        let title_x_offset = s(10.0) + badge_sz + s(6.0);
        let title_max_w = tab_w - title_x_offset - close_btn_sz - close_btn_pad - s(4.0);
        let title_max_chars = (title_max_w / ui_cw).floor().max(1.0) as usize;

        for (i, tab) in self.tabs.iter().enumerate() {
            let accent = self.accent_for(i);
            let hover_t = self.tab_hover_anim.get(i); // 0.0 → 1.0 smooth
            let active_t = self.active_anim.get(i);

            // Hover lift: inactive tabs shift up 1.5px on hover for physical "raise" feel.
            // Active tabs stay in place (they're already visually elevated via gradient/border).
            let lift_y = s(1.5) * hover_t * (1.0 - active_t);
            let rect = Rect {
                x: origin + i as f32 * (tab_w + tab_gap),
                y: bar.y + tab_inset - lift_y,
                width: tab_w,
                height: bar.height - tab_inset,
            };

            // Tab background — smoothly blend between inactive and active states
            let inactive_bg = lerp_color(colors::BG_DARK, colors::BG_SURFACE, hover_t);
            let bg = lerp_color(inactive_bg, colors::BG_BASE, active_t);

            let tab_radius = s(5.0);
            // Always render the tab background, blending between inactive and active states
            if active_t > 0.005 {
                // Active state (or transitioning toward it)
                let tab_top = [bg[0] * 1.18, bg[1] * 1.18, bg[2] * 1.18, 1.0];
                let border_alpha = lerp(0.0, 1.0, active_t);
                let border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], border_alpha];
                ui.fill_rounded_top_gradient(rect, tab_top, bg, tab_radius, active_t, border);

                // Ambient glow from accent color (fades in with active_t, reduced)
                let ambient_rect = Rect {
                    x: rect.x + 1.0, y: rect.y + 1.0,
                    width: rect.width - 2.0, height: rect.height - 1.0,
                };
                let inner_r = (tab_radius - 1.0).max(0.0);
                ui.fill_rounded_custom_gradient(ambient_rect,
                    [accent[0], accent[1], accent[2], 0.04 * active_t],
                    [accent[0], accent[1], accent[2], 0.0],
                    [inner_r, inner_r, 0.0, 0.0]);

                // Breathing glow (subtler — 0.92+0.08 range, halved alpha)
                let breath = 0.92 + 0.08 * self.glow_phase.sin();
                let glow_rect = Rect {
                    x: rect.x + tab_radius + s(4.0), y: rect.y - s(1.0),
                    width: rect.width - (tab_radius + s(4.0)) * 2.0, height: s(3.0),
                };
                ui.fill_shadow(glow_rect, [accent[0], accent[1], accent[2], 0.08 * breath * active_t], s(2.0), s(4.0));

                // Top accent bar — clean static line (fades in with active_t)
                let accent_bar = Rect {
                    x: rect.x + tab_radius,
                    y: rect.y + 1.0,
                    width: rect.width - tab_radius * 2.0,
                    height: s(2.0),
                };
                let accent_color = [accent[0], accent[1], accent[2], accent[3] * active_t];
                ui.fill_rounded(accent_bar, accent_color, s(1.0));
            }
            if active_t < 0.995 && hover_t > 0.005 {
                // Hover state for non-fully-active tabs
                let inv_active = 1.0 - active_t;
                let border_alpha = lerp(0.0, 0.6, hover_t) * inv_active;
                let hover_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], border_alpha];
                let top_boost = lerp(1.0, 1.06, hover_t);
                let hover_top = [bg[0] * top_boost, bg[1] * top_boost, bg[2] * top_boost, lerp(0.5, 1.0, hover_t) * inv_active];
                let hover_bottom = [bg[0], bg[1], bg[2], lerp(0.5, 1.0, hover_t) * inv_active];
                let radius = lerp(s(3.0), s(4.0), hover_t);
                let border_w = lerp(0.0, 0.5, hover_t) * inv_active;
                ui.fill_rounded_top_gradient(rect, hover_top, hover_bottom, radius, border_w, hover_border);
                // Faint accent preview line at the top of the hovered tab —
                // previews the active tab's accent color for this position.
                let preview_alpha = lerp(0.0, 0.35, hover_t) * inv_active;
                let preview_bar = Rect {
                    x: rect.x + radius + s(2.0),
                    y: rect.y + 1.0,
                    width: rect.width - (radius + s(2.0)) * 2.0,
                    height: s(2.0),
                };
                ui.fill_rounded(preview_bar,
                    [accent[0], accent[1], accent[2], preview_alpha],
                    s(1.0));
            }
            if active_t < 0.005 && hover_t < 0.005 {
                // Inactive rest state: clearly tab-shaped but receding.
                // Gradient (brighter top) gives subtle convex depth, and a
                // faint border defines the shape without competing with active.
                // Alpha 0.75 (up from 0.65) for better readability on dark bg.
                let rest_top = [
                    colors::BG_DARK[0] * 1.08, colors::BG_DARK[1] * 1.08,
                    colors::BG_DARK[2] * 1.08, 0.80,
                ];
                let rest_bot = [
                    colors::BG_DARK[0] * 1.03, colors::BG_DARK[1] * 1.03,
                    colors::BG_DARK[2] * 1.03, 0.80,
                ];
                let rest_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.28];
                // Subtle baseline shadow below inactive tab for physical depth
                let shadow_rect = Rect {
                    x: rect.x + s(4.0),
                    y: rect.bottom() - s(2.0),
                    width: rect.width - s(8.0),
                    height: s(3.0),
                };
                ui.fill_shadow(shadow_rect, [0.0, 0.0, 0.0, 0.08], s(2.0), s(4.0));
                ui.fill_rounded_top_gradient(rect, rest_top, rest_bot, s(3.0), 0.5, rest_border);
            }

            // Numbered circle badge — number rendered inside a colored circle
            // (matches opensessions reference where tabs show "1", "2", etc in colored circles)
            let num_str = format!("{}", i + 1);
            let badge_sz = ch * 0.9; // slightly smaller than line height
            let badge_x = rect.x + s(10.0);
            let badge_y = rect.y + (rect.height - badge_sz) / 2.0;
            let badge_r = badge_sz / 2.0;
            let badge_rect = Rect { x: badge_x, y: badge_y, width: badge_sz, height: badge_sz };

            // Subtle glow (only on active tab, much reduced)
            if active_t > 0.005 {
                let breath = 0.92 + 0.08 * self.glow_phase.sin();
                let glow_rect = Rect {
                    x: badge_x - s(2.0), y: badge_y - s(2.0),
                    width: badge_sz + s(4.0), height: badge_sz + s(4.0),
                };
                ui.fill_shadow(glow_rect,
                    [accent[0], accent[1], accent[2], 0.10 * breath * active_t],
                    badge_r + s(2.0), s(4.0));
            }

            // Circle background — flat solid fill (modern, clean)
            ui.fill_rounded(badge_rect, accent, badge_r);
            // Thin border for definition against dark background
            ui.stroke_rounded(badge_rect, badge_r, 0.5,
                [accent[0] * 0.7, accent[1] * 0.7, accent[2] * 0.7, 0.25]);

            // Number text (centered in circle, white on accent background for contrast)
            let num_w = text.text_width(&num_str);
            let num_x = badge_x + (badge_sz - num_w) / 2.0;
            let num_y = badge_y + (badge_sz - ch) / 2.0;
            ui.text(text, &num_str, num_x, num_y, [1.0, 1.0, 1.0, 1.0], accent);

            // Tab title (truncated to fit)
            // Inactive tabs start from a blend between FG_SECONDARY and FG_PRIMARY
            // for better readability against dark backgrounds
            let fg = lerp_color(
                lerp_color(
                    lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, 0.2),
                    colors::FG_PRIMARY,
                    hover_t * 0.4,
                ),
                colors::FG_PRIMARY,
                active_t,
            );
            let title = if title_max_chars > 2 {
                if tab.title.len() > title_max_chars {
                    format!("{}\u{2026}", &tab.title[..title_max_chars.saturating_sub(1)])
                } else {
                    tab.title.clone()
                }
            } else {
                String::new()
            };
            if !title.is_empty() {
                let title_x = rect.x + title_x_offset;
                if tab.active {
                    ui.text_ui_bold(text, &title, title_x, text_y(rect.height, rect.y), fg, bg);
                } else {
                    ui.text_ui(text, &title, title_x, text_y(rect.height, rect.y), fg, bg);
                }
            }

            // Close button — smoothly fades in based on active/hover state.
            // On active tabs the button is fully visible; on hovered inactive
            // tabs it fades in with the hover animation; on untouched inactive
            // tabs it's hidden. Uses smooth alpha for graceful transition.
            let close_fade = active_t.max(hover_t);
            if close_fade > 0.005 {
                let close_t = self.close_hover_anim.get(i);
                let close_rect = Rect {
                    x: rect.right() - close_btn_sz - close_btn_pad,
                    y: rect.y + (rect.height - close_btn_sz) / 2.0,
                    width: close_btn_sz,
                    height: close_btn_sz,
                };
                // Animated hover circle behind X icon (subtle red tint for destructive hint)
                if close_t > 0.005 {
                    // Red glow shadow behind close button for physical depth
                    let glow_rect = Rect {
                        x: close_rect.x - s(3.0), y: close_rect.y - s(3.0),
                        width: close_rect.width + s(6.0), height: close_rect.height + s(6.0),
                    };
                    ui.fill_shadow(glow_rect,
                        [colors::ACCENT_RED[0], colors::ACCENT_RED[1], colors::ACCENT_RED[2], 0.15 * close_t],
                        close_btn_sz / 2.0 + s(3.0), s(6.0));
                    let base = lerp_color(colors::BG_HOVER, colors::RED_SUBTLE, close_t * 0.6);
                    let hover_top = [
                        base[0] * 1.1, base[1] * 1.1,
                        base[2] * 1.1, base[3] * close_t,
                    ];
                    let hover_bot = [
                        base[0], base[1],
                        base[2], base[3] * close_t,
                    ];
                    ui.fill_rounded_gradient(close_rect, hover_top, hover_bot, close_btn_sz / 2.0);
                    // Subtle border on close hover circle
                    let close_border = [colors::ACCENT_RED[0], colors::ACCENT_RED[1], colors::ACCENT_RED[2], 0.15 * close_t];
                    ui.stroke_rounded(close_rect, close_btn_sz / 2.0, 0.5, close_border);
                }
                // Icon color: smoothly transition based on close hover + tab hover.
                // Uses close_fade for smooth visibility transition.
                let base_icon = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, active_t);
                let icon_color_base = lerp_color(base_icon, colors::FG_PRIMARY, close_t);
                let icon_color = [icon_color_base[0], icon_color_base[1], icon_color_base[2], icon_color_base[3] * close_fade];
                ui.icon_x(close_rect, s(7.0), icon_t, icon_color);
            }

            // Activity badge — pill-shaped notification with count, shown on
            // inactive tabs that have unread output.  Fades out when the tab
            // becomes active or when the close button is being hovered (avoids overlap).
            if tab.unread_count > 0 && active_t < 0.5 {
                let close_hover = self.close_hover_anim.get(i);
                let badge_fade = 1.0 - close_hover.max(active_t * 2.0);
                if badge_fade > 0.01 {
                    let count_str = if tab.unread_count > 99 { "99+".to_string() } else { tab.unread_count.to_string() };
                    let text_w = text.text_width(&count_str);
                    let badge_h = ch * 0.75;
                    let badge_pad = s(3.0);
                    // Pill width: at least a circle (for single digits), wider for multi-char
                    let badge_w = (text_w + badge_pad * 2.0).max(badge_h);
                    let badge_x = rect.right() - close_btn_pad - badge_w;
                    let badge_y = rect.y + s(5.0);
                    let badge_rect = Rect { x: badge_x, y: badge_y, width: badge_w, height: badge_h };
                    let badge_r = badge_h / 2.0;

                    // Subtle glow behind unread badge
                    let breath = 0.92 + 0.08 * self.glow_phase.sin();
                    let glow_rect = Rect {
                        x: badge_x - s(2.0), y: badge_y - s(2.0),
                        width: badge_w + s(4.0), height: badge_h + s(4.0),
                    };
                    ui.fill_shadow(
                        glow_rect,
                        [accent[0], accent[1], accent[2], 0.10 * breath * badge_fade],
                        badge_r, s(4.0),
                    );

                    // Pill body: flat solid fill (matching flattened tab badges)
                    let badge_color = [accent[0], accent[1], accent[2], badge_fade];
                    let badge_border = [accent[0] * 0.7, accent[1] * 0.7, accent[2] * 0.7, 0.25 * badge_fade];
                    ui.fill_rounded(badge_rect, badge_color, badge_r);
                    ui.stroke_rounded(badge_rect, badge_r, 0.5, badge_border);

                    // Count text (centered in pill) — white on accent background for max contrast
                    let text_x = badge_x + (badge_w - text_w) / 2.0;
                    let text_y = badge_y + (badge_h - ch) / 2.0;
                    let text_color = [1.0, 1.0, 1.0, badge_fade];
                    ui.text(text, &count_str, text_x, text_y, text_color, accent);
                }
            }

            // Right separator between tabs — embossed groove for depth
            // (matches sidebar groove language: dark edge + light highlight pair)
            if !tab.active && i + 1 < self.tabs.len() {
                let next_active = self.tabs.get(i + 1).map_or(false, |t| t.active);
                if !next_active {
                    // Fade separator out when either adjacent tab is hovered
                    let next_hover = self.tab_hover_anim.get(i + 1);
                    let sep_fade = 1.0 - (hover_t.max(next_hover));
                    let groove_dark = [0.0, 0.0, 0.0, 0.12 * sep_fade];
                    let groove_light = [1.0, 1.0, 1.0, 0.04 * sep_fade];
                    ui.vgroove_fade(rect.right(), rect.y + s(6.0), rect.height - s(12.0),
                        groove_dark, groove_light, s(6.0));
                }
            }
        }

        // "+ New tab" button after last tab — subtle pill icon button
        let new_t = self.new_tab_anim.value();
        let new_x = origin + self.tabs.len() as f32 * (tab_w + tab_gap) + s(8.0);
        let btn_sz = s(24.0);
        let new_btn_y = bar.y + (bar.height - btn_sz) / 2.0;
        let new_rect = Rect { x: new_x, y: new_btn_y, width: btn_sz, height: btn_sz };
        let btn_radius = btn_sz / 2.0;
        if new_t > 0.005 {
            // Hover: brightening circular background
            let new_bg = lerp_color(
                [colors::BG_DARK[0] * 1.06, colors::BG_DARK[1] * 1.06, colors::BG_DARK[2] * 1.06, 0.5],
                colors::BG_SURFACE,
                new_t,
            );
            let new_top = [new_bg[0] * lerp(1.0, 1.10, new_t), new_bg[1] * lerp(1.0, 1.10, new_t), new_bg[2] * lerp(1.0, 1.10, new_t), new_bg[3]];
            let border_alpha = lerp(0.15, 0.6, new_t);
            let border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], border_alpha];
            ui.fill_rounded_gradient(new_rect, new_top, new_bg, btn_radius);
            ui.stroke_rounded(new_rect, btn_radius, 0.5, border);
        } else {
            // Rest: subtle circular border for discoverable icon button
            let rest_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.18];
            ui.stroke_rounded(new_rect, btn_radius, 0.5, rest_border);
        }
        let new_tab_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, new_t);
        ui.icon_plus(new_rect, icon_t, s(5.0), new_tab_fg);

        // Window control buttons (minimize, maximize, close) — animated hovers
        let buttons = self.window_button_rects(bar, text.scale);

        // Subtle separator before window controls — visual boundary between
        // tab content and window chrome buttons, fading at top/bottom for softness
        {
            let sep_x = buttons[0].0.x - s(4.0);
            let sep_pad = s(8.0);
            ui.vline_fade(sep_x, bar.y + sep_pad, bar.height - sep_pad * 2.0, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.3], s(6.0));
        }
        for (rect, btn) in &buttons {
            let btn_t = match btn {
                WindowButton::Minimize => self.btn_minimize_anim.value(),
                WindowButton::Maximize => self.btn_maximize_anim.value(),
                WindowButton::Close => self.btn_close_anim.value(),
            };

            if btn_t > 0.005 {
                let (base_color, border_color) = if *btn == WindowButton::Close {
                    (
                        [colors::ACCENT_RED[0], colors::ACCENT_RED[1], colors::ACCENT_RED[2], colors::ACCENT_RED[3] * btn_t],
                        [colors::ACCENT_RED[0] * 0.7, colors::ACCENT_RED[1] * 0.7, colors::ACCENT_RED[2] * 0.7, 0.6 * btn_t],
                    )
                } else {
                    (
                        [colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2], colors::BG_HOVER[3] * btn_t],
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.4 * btn_t],
                    )
                };
                let hover_top = [base_color[0] * 1.12, base_color[1] * 1.12, base_color[2] * 1.12, base_color[3]];
                ui.fill_rounded_gradient(*rect, hover_top, base_color, s(3.0));
                ui.stroke_rounded(*rect, s(3.0), 0.5, border_color);
            }

            let icon_color = if *btn == WindowButton::Close {
                lerp_color(colors::FG_MUTED, colors::WHITE, btn_t)
            } else {
                lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, btn_t)
            };
            match btn {
                WindowButton::Minimize => ui.icon_minimize(*rect, s(10.0), icon_t, icon_color),
                WindowButton::Maximize => ui.icon_maximize(*rect, s(9.0), icon_t, icon_color),
                WindowButton::Close => ui.icon_x(*rect, s(9.0), icon_t, icon_color),
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect, scale: f32) -> Option<UiAction> {
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_tab = None;
                self.hovered_close_tab = None;
                self.hovered_button = None;
                self.hovered_new_tab = self.new_tab_rect(bar, scale).contains(x, y);
                let close_btn_sz = 16.0 * scale;
                let close_btn_pad = 8.0 * scale;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let rect = self.tab_rect(i, bar, scale);
                    if rect.contains(x, y) {
                        self.hovered_tab = Some(i);
                        // Check close button hover (visible on active + hovered tabs)
                        let show_close = tab.active || true; // hovered_tab is already Some(i)
                        if show_close {
                            let close_rect = Rect {
                                x: rect.right() - close_btn_sz - close_btn_pad,
                                y: rect.y + (rect.height - close_btn_sz) / 2.0,
                                width: close_btn_sz,
                                height: close_btn_sz,
                            };
                            if close_rect.contains(x, y) {
                                self.hovered_close_tab = Some(i);
                            }
                        }
                    }
                }
                for (rect, btn) in &self.window_button_rects(bar, scale) {
                    if rect.contains(x, y) {
                        self.hovered_button = Some(*btn);
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                // Check window buttons first
                for (rect, btn) in &self.window_button_rects(bar, scale) {
                    if rect.contains(x, y) {
                        return match btn {
                            WindowButton::Close => Some(UiAction::CloseWindow),
                            WindowButton::Minimize => Some(UiAction::MinimizeWindow),
                            WindowButton::Maximize => Some(UiAction::MaximizeWindow),
                        };
                    }
                }
                // Check tabs — close button takes priority over tab switch
                let close_btn_sz = 16.0 * scale;
                let close_btn_pad = 8.0 * scale;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let rect = self.tab_rect(i, bar, scale);
                    if rect.contains(x, y) {
                        // Check if click landed on the close button area
                        let close_rect = Rect {
                            x: rect.right() - close_btn_sz - close_btn_pad,
                            y: rect.y + (rect.height - close_btn_sz) / 2.0,
                            width: close_btn_sz,
                            height: close_btn_sz,
                        };
                        if close_rect.contains(x, y) {
                            return Some(UiAction::CloseTab(tab.id.clone()));
                        }
                        return Some(UiAction::SwitchTab(tab.id.clone()));
                    }
                }
                // Click on empty tab bar area = drag window
                if bar.contains(x, y) {
                    return Some(UiAction::DragWindow);
                }
                None
            }
            _ => None,
        }
    }
}
