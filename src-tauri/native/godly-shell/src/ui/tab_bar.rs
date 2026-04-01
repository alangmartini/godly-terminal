//! Tab bar: horizontal row of tab buttons with numbered indicators.
//! Also serves as the title bar (window drag + min/max/close buttons).

use super::anim::{self, Anim, AnimVec, lerp_color, lerp};
use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const TAB_MAX_WIDTH: f32 = 170.0;
const TAB_MIN_WIDTH: f32 = 90.0;
const TAB_GAP: f32 = 6.0;
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
    /// Optional per-tab accent color override. When `Some`, used instead of
    /// the index-based rotation from `TAB_ACCENTS`.
    pub accent: Option<[f32; 4]>,
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
        // new-tab button + gap before window controls + right-side process indicators
        let new_tab_reserve = (36.0 * scale).round();
        let sep_reserve = (12.0 * scale).round();
        let indicators_reserve = (150.0 * scale).round(); // space for right-side process labels
        let avail = bar.right() - origin - btn_reserve - new_tab_reserve - sep_reserve - indicators_reserve;
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

    pub fn accent_for(&self, index: usize) -> [f32; 4] {
        self.tabs.get(index)
            .and_then(|t| t.accent)
            .unwrap_or_else(|| TAB_ACCENTS[index % TAB_ACCENTS.len()])
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

        // Background — flat fill matching web reference (background: #0f1117)
        ui.fill(bar, colors::BG_RAISED);

        // (No top accent stripe — clean flat bar matching web reference)

        // Bottom separator — solid 1px hairline matching web reference
        // (borderBottom: "1px solid #1a1d25").
        ui.hline_aa(bar.x, bar.bottom() - 1.0, bar.width, 1.0, colors::BORDER);

        // Sidebar section: "Godly Terminal" branding
        if self.sidebar_width > 0.0 {
            // Flat background matching sidebar tone (web reference: same #0b0d12)
            let brand_section = Rect {
                x: bar.x, y: bar.y,
                width: self.sidebar_width, height: bar.height,
            };
            ui.fill(brand_section, colors::BG_DARK);
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
            // Right border for sidebar section — solid hairline matching web
            ui.vline(self.sidebar_width - 1.0, bar.y, bar.height, 1.0, colors::BORDER);
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
        // Badge is 18px circle at x+10, then gap before title — web: width 18, height 18
        let badge_sz = s(18.0);
        let title_x_offset = s(10.0) + badge_sz + s(6.0);
        let title_max_w = tab_w - title_x_offset - close_btn_sz - close_btn_pad - s(4.0);
        let title_max_chars = (title_max_w / ui_cw).floor().max(1.0) as usize;

        for (i, tab) in self.tabs.iter().enumerate() {
            let accent = self.accent_for(i);
            let hover_t = self.tab_hover_anim.get(i); // 0.0 → 1.0 smooth
            let active_t = self.active_anim.get(i);

            // Flat tabs — no hover lift (matching web reference)
            let rect = Rect {
                x: origin + i as f32 * (tab_w + tab_gap),
                y: bar.y + tab_inset,
                width: tab_w,
                height: bar.height - tab_inset,
            };

            // Tab background — web reference style: flat rects, no rounded-top gradients.
            // Active: #161920 background + 2px colored bottom border
            // Hover: subtle background lift
            // Inactive: transparent
            let bg = lerp_color(
                lerp_color(colors::BG_RAISED, colors::BG_SURFACE, hover_t),
                colors::BG_TAB_ACTIVE,
                active_t,
            );

            if active_t > 0.005 {
                // Active: flat background matching web (#161920)
                let active_bg = [colors::BG_TAB_ACTIVE[0], colors::BG_TAB_ACTIVE[1], colors::BG_TAB_ACTIVE[2], active_t];
                ui.fill(rect, active_bg);

                // Bottom accent indicator (2px colored bar) — web: borderBottom: 2px solid ${color}
                let accent_color = [accent[0], accent[1], accent[2], active_t];
                let bottom_bar = Rect {
                    x: rect.x,
                    y: rect.bottom() - s(2.0),
                    width: rect.width,
                    height: s(2.0),
                };
                ui.fill(bottom_bar, accent_color);
            }
            if active_t < 0.995 && hover_t > 0.005 {
                // Hover: subtle flat background
                let inv_active = 1.0 - active_t;
                let hover_bg = [
                    colors::BG_SURFACE[0], colors::BG_SURFACE[1],
                    colors::BG_SURFACE[2], hover_t * 0.5 * inv_active,
                ];
                ui.fill(rect, hover_bg);
            }

            // Numbered circle badge — web: width 18, height 18, borderRadius "50%",
            //   background "${color}22", color: tab.color, fontSize 10, fontWeight 700
            let num_str = format!("{}", i + 1);
            let badge_sz = s(18.0);
            let badge_x = rect.x + s(10.0);
            let badge_y = rect.y + (rect.height - badge_sz) / 2.0;
            let badge_r = badge_sz / 2.0;
            let badge_rect = Rect { x: badge_x, y: badge_y, width: badge_sz, height: badge_sz };

            // Circle background — semi-transparent accent overlay (web: ${color}22 = 13%)
            let badge_bg = [accent[0], accent[1], accent[2], 0.13];
            ui.fill_rounded(badge_rect, badge_bg, badge_r);

            // Number text — proportional font, centered in circle
            // Web: fontSize 10, fontWeight 700
            let num_w = text.text_width_ui_scaled(&num_str, font_scale::PX10);
            let num_ch = ch * font_scale::PX10;
            let num_x = badge_x + (badge_sz - num_w) / 2.0;
            let num_y = badge_y + (badge_sz - num_ch) / 2.0;
            ui.text_ui_scaled(text, &num_str, num_x, num_y, accent, badge_bg, font_scale::PX10);

            // Tab title (truncated to fit)
            // Web: fontSize 12, fontWeight active ? 600 : 400
            let fg = lerp_color(
                lerp_color(colors::FG_DIM, colors::FG_SECONDARY, hover_t),
                colors::FG_BRIGHT,
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
                let title_y = text_y(rect.height, rect.y);
                if tab.active {
                    ui.text_ui_bold_scaled(text, &title, title_x, title_y, fg, bg, font_scale::PX12);
                } else {
                    ui.text_ui_scaled(text, &title, title_x, title_y, fg, bg, font_scale::PX12);
                }
            }

            // Close button — always faintly visible for discoverability.
            // Active tabs: fully visible. Hovered inactive: fades in with hover.
            // Rest inactive: very faint (0.18 alpha) so users know it's there.
            let close_fade = active_t.max(hover_t).max(0.18);
            {
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

            // Activity badge — rounded notification with count, shown on
            // inactive tabs that have unread output.  Fades out when the tab
            // becomes active or when the close button is being hovered (avoids overlap).
            // Web reference: height 16, borderRadius 7, fontSize 9, padding "1px 5px", minWidth 16
            if tab.unread_count > 0 {
                let close_hover = self.close_hover_anim.get(i);
                let badge_fade = 1.0 - close_hover;
                if badge_fade > 0.01 {
                    let count_str = if tab.unread_count > 99 { "99+".to_string() } else { tab.unread_count.to_string() };
                    let text_w = text.text_width(&count_str);
                    let badge_h = s(16.0); // web: height 16
                    let badge_pad = s(5.0); // web: padding "1px 5px"
                    let badge_w = (text_w + badge_pad * 2.0).max(s(16.0)); // web: minWidth 16
                    let badge_x = rect.right() - close_btn_pad - badge_w;
                    let badge_y = rect.y + s(5.0);
                    let badge_rect = Rect { x: badge_x, y: badge_y, width: badge_w, height: badge_h };
                    let badge_r = s(7.0); // web: borderRadius 7 (not full pill)

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

            // Right separator between tabs — single thin hairline
            // Modern approach: surface color difference provides primary separation,
            // the hairline is just a subtle visual cue that fades on hover.
            if !tab.active && i + 1 < self.tabs.len() {
                let next_active = self.tabs.get(i + 1).map_or(false, |t| t.active);
                if !next_active {
                    let next_hover = self.tab_hover_anim.get(i + 1);
                    let sep_fade = 1.0 - (hover_t.max(next_hover));
                    ui.vline_fade(rect.right(), rect.y + s(8.0), rect.height - s(16.0), 1.0,
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.15 * sep_fade],
                        s(8.0));
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
                [colors::BG_RAISED[0] * 1.06, colors::BG_RAISED[1] * 1.06, colors::BG_RAISED[2] * 1.06, 0.5],
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
        let new_tab_fg = lerp_color(colors::FG_DIM, colors::FG_SECONDARY, new_t);
        ui.icon_plus(new_rect, icon_t, s(5.0), new_tab_fg);

        // Right-side process indicators — web: display flex, gap 10, paddingRight 14,
        // fontSize 11, color "#555d6b", fontWeight 600
        // Shows active processes: 🟠 bun, ● opensessions
        let buttons = self.window_button_rects(bar, text.scale);
        {
            let indicator_color = colors::FG_DIM; // #555d6b
            let indicator_gap = s(10.0);
            let indicator_pad_r = s(14.0);
            let indicators_x_end = buttons[0].0.x - s(8.0); // before separator
            let ind_ch = ch * font_scale::PX11;
            let iy = bar.y + (bar.height - ind_ch) / 2.0;

            // "opensessions" with green dot — web: fontSize 11, fontWeight 600
            let opensessions_label = "opensessions";
            let opensessions_w = text.text_width_ui_scaled(opensessions_label, font_scale::PX11);
            let dot_sz = s(8.0);
            let dot_gap = s(4.0);
            let opensessions_total = dot_sz + dot_gap + opensessions_w;
            let opensessions_x = indicators_x_end - indicator_pad_r - opensessions_total;
            // Green dot (8x8, borderRadius 50%, #22c55e)
            let dot_y = bar.y + (bar.height - dot_sz) / 2.0;
            ui.fill_rounded(
                Rect { x: opensessions_x, y: dot_y, width: dot_sz, height: dot_sz },
                colors::ACCENT_GREEN, dot_sz / 2.0,
            );
            ui.text_ui_bold_scaled(text, opensessions_label,
                opensessions_x + dot_sz + dot_gap, iy,
                indicator_color, colors::BG_RAISED, font_scale::PX11);

            // "bun" with orange emoji dot — web: fontSize 11, fontWeight 600
            let bun_label = "bun";
            let bun_w = text.text_width_ui_scaled(bun_label, font_scale::PX11);
            let bun_dot_sz = s(8.0);
            let bun_total = bun_dot_sz + dot_gap + bun_w;
            let bun_x = opensessions_x - indicator_gap - bun_total;
            // Orange dot (approximating 🟠 emoji with orange circle)
            ui.fill_rounded(
                Rect { x: bun_x, y: dot_y, width: bun_dot_sz, height: bun_dot_sz },
                colors::ACCENT_PEACH, bun_dot_sz / 2.0,
            );
            ui.text_ui_bold_scaled(text, bun_label,
                bun_x + bun_dot_sz + dot_gap, iy,
                indicator_color, colors::BG_RAISED, font_scale::PX11);
        }

        // Subtle separator before window controls — softer to match quieter aesthetic
        {
            let sep_x = buttons[0].0.x - s(4.0);
            let sep_pad = s(8.0);
            ui.vline_fade(sep_x, bar.y + sep_pad, bar.height - sep_pad * 2.0, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.20], s(8.0));
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
                lerp_color(colors::FG_DIM, colors::WHITE, btn_t)
            } else {
                lerp_color(colors::FG_DIM, colors::FG_SECONDARY, btn_t)
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
