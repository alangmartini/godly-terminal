//! Tab bar: horizontal row of tab buttons with numbered indicators.
//! Also serves as the title bar (window drag + min/max/close buttons).

use std::cell::RefCell;

use super::anim::{self, lerp_color, Anim, AnimVec};
use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::text_layout::FontWeight;
use super::tab_bar_layout::{TabBarLayout, TabBarLayoutConfig, TabBarLayoutEngine};
use super::widget::{MouseEvent, Rect, UiAction};

pub(super) const TAB_MAX_WIDTH: f32 = 170.0;
pub(super) const TAB_MIN_WIDTH: f32 = 90.0;
pub(super) const TAB_GAP: f32 = 6.0;
pub(super) const TAB_MARGIN_LEFT: f32 = 6.0;
pub(super) const TAB_INSET_V: f32 = 3.0;
pub(super) const BUTTON_WIDTH: f32 = 46.0;
const ICON_LINE_T: f32 = 1.2;
const CROP_TAB_TITLE_SCALE: f32 = 12.8 / 14.0;
const CROP_TAB_BADGE_TEXT_SCALE: f32 = 9.5 / 14.0;
const CROP_TAB_BADGE_SIZE: f32 = 19.0;
const CROP_TAB_PAD_X: f32 = 15.0;

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
    pub show_brand: bool,
    pub show_indicators: bool,
    pub show_window_controls: bool,
    pub show_new_tab_button: bool,
    pub show_tab_close_buttons: bool,
    pub content_sized_tabs: bool,
    /// When true, use reference-capture-specific font scales and sizes
    /// (CROP_TAB_* constants) for screenshot parity. When false, use
    /// standard PX12/PX9 scales even in content-sized mode.
    pub crop_mode: bool,
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
    layout_engine: RefCell<TabBarLayoutEngine>,
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
            show_brand: false,
            show_indicators: true,
            show_window_controls: true,
            show_new_tab_button: true,
            show_tab_close_buttons: true,
            content_sized_tabs: true,
            crop_mode: false,
            tab_hover_anim: AnimVec::default(),
            close_hover_anim: AnimVec::default(),
            new_tab_anim: Anim::default(),
            btn_minimize_anim: Anim::default(),
            btn_maximize_anim: Anim::default(),
            btn_close_anim: Anim::default(),
            active_anim: AnimVec::default(),
            glow_phase: 0.0,
            layout_engine: RefCell::new(TabBarLayoutEngine::new()),
        }
    }

    /// Advance all hover animations. `dt` = seconds since last frame. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        // Update targets from current hover state
        self.tab_hover_anim.ensure_len(self.tabs.len());
        self.close_hover_anim.ensure_len(self.tabs.len());
        for i in 0..self.tabs.len() {
            self.tab_hover_anim.set(
                i,
                if self.hovered_tab == Some(i) {
                    1.0
                } else {
                    0.0
                },
            );
            self.close_hover_anim.set(
                i,
                if self.hovered_close_tab == Some(i) {
                    1.0
                } else {
                    0.0
                },
            );
        }
        self.active_anim.ensure_len(self.tabs.len());
        for i in 0..self.tabs.len() {
            self.active_anim
                .set(i, if self.tabs[i].active { 1.0 } else { 0.0 });
        }
        self.new_tab_anim
            .set(if self.hovered_new_tab { 1.0 } else { 0.0 });
        self.btn_minimize_anim
            .set(if self.hovered_button == Some(WindowButton::Minimize) {
                1.0
            } else {
                0.0
            });
        self.btn_maximize_anim
            .set(if self.hovered_button == Some(WindowButton::Maximize) {
                1.0
            } else {
                0.0
            });
        self.btn_close_anim
            .set(if self.hovered_button == Some(WindowButton::Close) {
                1.0
            } else {
                0.0
            });

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
            if self.glow_phase > std::f32::consts::TAU {
                self.glow_phase -= std::f32::consts::TAU;
            }
            animating = true;
        }

        animating
    }

    fn layout(&self, bar: Rect, scale: f32, text: Option<&UiTextRenderer>) -> TabBarLayout {
        let tab_widths = if self.content_sized_tabs {
            self.intrinsic_tab_widths(scale, text)
        } else {
            Vec::new()
        };
        self.layout_engine.borrow_mut().compute(
            bar,
            self.sidebar_width,
            self.tabs.len(),
            &tab_widths,
            scale,
            TabBarLayoutConfig {
                show_brand: self.show_brand,
                show_indicators: self.show_indicators,
                show_controls: self.show_window_controls,
                show_new_tab: self.show_new_tab_button,
                content_sized_tabs: self.content_sized_tabs,
                tabs_padding_left: if self.content_sized_tabs {
                    2.0
                } else {
                    TAB_MARGIN_LEFT
                },
                tab_inset_v: if self.content_sized_tabs {
                    0.0
                } else {
                    TAB_INSET_V
                },
            },
        )
    }

    fn intrinsic_tab_widths(&self, scale: f32, text: Option<&UiTextRenderer>) -> Vec<f32> {
        let s = |v: f32| (v * scale).round();
        let title_scale = if self.crop_mode {
            CROP_TAB_TITLE_SCALE
        } else {
            font_scale::PX12
        };
        let badge_text_scale = if self.crop_mode {
            CROP_TAB_BADGE_TEXT_SCALE
        } else {
            font_scale::PX9
        };
        let badge_size = if self.crop_mode {
            CROP_TAB_BADGE_SIZE
        } else {
            16.0
        };
        let leading_pad = if self.crop_mode {
            CROP_TAB_PAD_X
        } else {
            14.0
        };
        let text_width = |value: &str, text: Option<&UiTextRenderer>, font_scale: f32| {
            if let Some(renderer) = text {
                // Use SemiBold (600) for width measurement — active tabs render at
                // SemiBold, and we need the slot to fit the widest case.
                renderer.text_width_ui_weighted_scaled(value, font_scale, FontWeight::SemiBold)
            } else {
                value.chars().count() as f32 * (7.0 * scale) * font_scale
            }
        };

        self.tabs
            .iter()
            .map(|tab| {
                let title_w = text_width(&tab.title, text, title_scale);
                let badge_w = if tab.unread_count > 0 {
                    let badge_text = if tab.unread_count > 99 {
                        "99+".to_string()
                    } else {
                        tab.unread_count.to_string()
                    };
                    let text_w = text_width(&badge_text, text, badge_text_scale);
                    (text_w + s(5.0) * 2.0).max(s(badge_size)) + s(6.0)
                } else {
                    0.0
                };

                let circle_sz = if self.crop_mode { CROP_TAB_BADGE_SIZE } else { 18.0 };
                let close_btn_space = if self.show_tab_close_buttons { s(16.0) + s(8.0) } else { 0.0 };
                s(leading_pad) + s(circle_sz) + s(6.0) + title_w + badge_w + close_btn_space + s(4.0)
            })
            .collect()
    }

    /// Current breathing glow phase (for sharing with other components).
    pub fn glow_phase(&self) -> f32 {
        self.glow_phase
    }

    pub fn accent_for(&self, index: usize) -> [f32; 4] {
        self.tabs
            .get(index)
            .and_then(|t| t.accent)
            .unwrap_or_else(|| TAB_ACCENTS[index % TAB_ACCENTS.len()])
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;
        let text_y = |area_h: f32, y: f32| y + (area_h - ch) / 2.0;
        let layout = self.layout(bar, text.scale, Some(text));

        // Background — flat fill matching web reference (background: #0f1117)
        ui.fill(bar, colors::BG_RAISED);

        // (No top accent stripe — clean flat bar matching web reference)

        // Bottom separator — solid 1px hairline matching web reference
        // (borderBottom: "1px solid #1a1d25").
        ui.hline_aa(bar.x, bar.bottom() - 1.0, bar.width, 1.0, colors::BORDER);

        // Sidebar section: "Godly Terminal" branding
        if self.show_brand && layout.brand.width > 0.0 {
            // Flat background matching sidebar tone (web reference: same #0b0d12)
            let brand_section = layout.brand;
            ui.fill(brand_section, colors::BG_DARK);
            let icon_size = ch * 1.1;
            let icon_x = brand_section.x + s(10.0);
            let icon_y = brand_section.y + (brand_section.height - icon_size) / 2.0;
            let brand_x = icon_x + icon_size + s(6.0);
            let brand_y = text_y(brand_section.height, brand_section.y);
            // Branding text with subtle accent tint from active tab color
            let active_accent = self
                .tabs
                .iter()
                .enumerate()
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
                Rect {
                    x: icon_x,
                    y: icon_y,
                    width: icon_size,
                    height: icon_size,
                },
                icon_t,
                icon_fg,
            );
            ui.text_ui_bold(
                text,
                "Godly Terminal",
                brand_x,
                brand_y,
                brand_fg,
                colors::BG_DARK,
            );
            // Right border for sidebar section — solid hairline matching web
            ui.vline(
                brand_section.right() - 1.0,
                brand_section.y,
                brand_section.height,
                1.0,
                colors::BORDER,
            );
        }

        // Icon line thickness (used for close buttons in tabs and window controls)
        let icon_t = (ICON_LINE_T * text.scale).max(1.0);

        // Close button dimensions (reserved on right side of every tab)
        let close_btn_sz = if self.show_tab_close_buttons {
            s(16.0)
        } else {
            0.0
        };
        let close_btn_pad = if self.show_tab_close_buttons {
            s(8.0)
        } else {
            0.0
        };
        let tab_pad_x = if self.crop_mode {
            s(CROP_TAB_PAD_X)
        } else {
            s(14.0) // web: padding "0 14px"
        };

        // Proportional UI font advance for tab title width estimation.
        // Using ui_avg_advance gives tighter truncation than monospace cell_width
        // because proportional characters are narrower on average (~75% of cell_width).
        let ui_cw = if text.ui_avg_advance > 0.0 {
            text.ui_avg_advance
        } else {
            cw * 0.75
        };

        // Title metrics within each retained-layout tab slot.
        let badge_sz = if self.crop_mode {
            s(CROP_TAB_BADGE_SIZE)
        } else {
            s(18.0)
        };
        let title_x_offset = tab_pad_x + badge_sz + s(6.0);

        for (i, tab) in self.tabs.iter().enumerate() {
            let accent = self.accent_for(i);
            let hover_t = self.tab_hover_anim.get(i); // 0.0 → 1.0 smooth
            let active_t = self.active_anim.get(i);

            let rect = layout.tabs[i];
            let title_max_w = rect.width - title_x_offset - close_btn_sz - close_btn_pad - s(4.0);
            let title_max_chars = (title_max_w / ui_cw).floor().max(1.0) as usize;

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
                // Active: flat background + 2px bottom accent border as single SDF quad
                // Web: backgroundColor "#161920", borderBottom: "2px solid ${accentColor}"
                let active_bg = [
                    colors::BG_TAB_ACTIVE[0],
                    colors::BG_TAB_ACTIVE[1],
                    colors::BG_TAB_ACTIVE[2],
                    active_t,
                ];
                let accent_color = [accent[0], accent[1], accent[2], active_t];
                // Per-side border: [top, right, bottom, left] — only bottom side
                ui.fill_rounded_border_sides(
                    rect,
                    active_bg,
                    0.0,
                    [0.0, 0.0, s(2.0), 0.0],
                    accent_color,
                );
            }
            if active_t < 0.995 && hover_t > 0.005 {
                // Hover: subtle flat background
                let inv_active = 1.0 - active_t;
                let hover_bg = [
                    colors::BG_SURFACE[0],
                    colors::BG_SURFACE[1],
                    colors::BG_SURFACE[2],
                    hover_t * 0.5 * inv_active,
                ];
                ui.fill(rect, hover_bg);
            }

            // Numbered circle badge — web: width 18, height 18, borderRadius "50%",
            //   background "${color}22", color: tab.color, fontSize 10, fontWeight 700
            let num_str = format!("{}", i + 1);
            let badge_sz = if self.crop_mode {
                s(CROP_TAB_BADGE_SIZE)
            } else {
                s(18.0)
            };
            let badge_x = rect.x + tab_pad_x;
            let badge_y = rect.y + (rect.height - badge_sz) / 2.0;
            let badge_r = badge_sz / 2.0;
            let badge_rect = Rect {
                x: badge_x,
                y: badge_y,
                width: badge_sz,
                height: badge_sz,
            };

            // Circle background — semi-transparent accent overlay (web: ${color}22 = 13%)
            let badge_bg = [accent[0], accent[1], accent[2], 0.13];
            ui.fill_rounded(badge_rect, badge_bg, badge_r);

            // Number text — proportional font, centered in circle
            // Web: fontSize 10, fontWeight 700
            let badge_num_scale = if self.crop_mode {
                10.5 / 14.0
            } else {
                font_scale::PX10
            };
            let num_w = text.text_width_ui_weighted_scaled(&num_str, badge_num_scale, FontWeight::Bold);
            let num_ch = ch * badge_num_scale;
            let num_x = badge_x + (badge_sz - num_w) / 2.0;
            let num_y = badge_y + (badge_sz - num_ch) / 2.0;
            ui.text_ui_bold_scaled(
                text,
                &num_str,
                num_x,
                num_y,
                accent,
                badge_bg,
                badge_num_scale,
            );

            // Tab title (truncated to fit)
            // Web: fontSize 12, fontWeight active ? 600 : 400
            let fg = lerp_color(
                lerp_color(colors::FG_DIM, colors::FG_SECONDARY, hover_t),
                colors::FG_BRIGHT,
                active_t,
            );
            let title = if self.content_sized_tabs {
                tab.title.clone()
            } else if title_max_chars > 2 {
                if tab.title.len() > title_max_chars {
                    format!(
                        "{}\u{2026}",
                        &tab.title[..title_max_chars.saturating_sub(1)]
                    )
                } else {
                    tab.title.clone()
                }
            } else {
                String::new()
            };
            if !title.is_empty() {
                let title_x = rect.x + title_x_offset;
                let title_y = text_y(rect.height, rect.y);
                let title_scale = if self.crop_mode {
                    CROP_TAB_TITLE_SCALE
                } else {
                    font_scale::PX12
                };
                let title_w = if tab.active {
                    text.text_width_ui_weighted_scaled(&title, title_scale, FontWeight::SemiBold)
                } else {
                    text.text_width_ui_scaled(&title, title_scale)
                };
                if tab.active {
                    ui.text_ui_semibold_scaled(text, &title, title_x, title_y, fg, bg, title_scale);
                } else {
                    ui.text_ui_scaled(text, &title, title_x, title_y, fg, bg, title_scale);
                }

                // Activity badge — inline in the crop/reference layout, right-aligned in the
                // live shell layout where slots are stretched across the strip.
                if tab.unread_count > 0 {
                    let close_hover = if self.show_tab_close_buttons {
                        self.close_hover_anim.get(i)
                    } else {
                        0.0
                    };
                    let badge_fade = 1.0 - close_hover;
                    if badge_fade > 0.01 {
                        let count_str = if tab.unread_count > 99 {
                            "99+".to_string()
                        } else {
                            tab.unread_count.to_string()
                        };
                        let badge_text_scale = if self.crop_mode {
                            CROP_TAB_BADGE_TEXT_SCALE
                        } else {
                            font_scale::PX9
                        };
                        let text_w = text.text_width_ui_weighted_scaled(&count_str, badge_text_scale, FontWeight::Bold);
                        let badge_h = if self.crop_mode {
                            s(17.0)
                        } else {
                            s(16.0)
                        };
                        let badge_pad = s(5.0);
                        let badge_w = (text_w + badge_pad * 2.0).max(s(16.0));
                        let badge_x = if self.content_sized_tabs {
                            title_x + title_w + s(6.0)
                        } else {
                            rect.right() - close_btn_pad - badge_w
                        };
                        let badge_y = if self.content_sized_tabs {
                            rect.y + (rect.height - badge_h) / 2.0
                        } else {
                            rect.y + s(5.0)
                        };
                        let badge_rect = Rect {
                            x: badge_x,
                            y: badge_y,
                            width: badge_w,
                            height: badge_h,
                        };
                        let badge_r = s(7.0);

                        let badge_color = [accent[0], accent[1], accent[2], badge_fade];
                        ui.fill_rounded(badge_rect, badge_color, badge_r);

                        let badge_ch = ch * badge_text_scale;
                        let text_x = badge_x + (badge_w - text_w) / 2.0;
                        let text_y = badge_y + (badge_h - badge_ch) / 2.0;
                        let text_color = [1.0, 1.0, 1.0, badge_fade];
                        ui.text_ui_bold_scaled(
                            text,
                            &count_str,
                            text_x,
                            text_y,
                            text_color,
                            accent,
                            badge_text_scale,
                        );
                    }
                }
            }

            // Close button — always faintly visible for discoverability.
            // Active tabs: fully visible. Hovered inactive: fades in with hover.
            // Rest inactive: very faint (0.18 alpha) so users know it's there.
            let close_fade = active_t.max(hover_t).max(0.18);
            if self.show_tab_close_buttons {
                let close_t = self.close_hover_anim.get(i);
                let close_rect = Rect {
                    x: rect.right() - close_btn_sz - close_btn_pad,
                    y: rect.y + (rect.height - close_btn_sz) / 2.0,
                    width: close_btn_sz,
                    height: close_btn_sz,
                };
                if close_t > 0.005 {
                    // Flat hover fill — no glow, no gradient, no border
                    let close_bg = lerp_color(colors::BG_HOVER, colors::RED_SUBTLE, close_t * 0.6);
                    let close_hover = [close_bg[0], close_bg[1], close_bg[2], close_bg[3] * close_t];
                    ui.fill_rounded(close_rect, close_hover, close_btn_sz / 2.0);
                }
                let base_icon = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, active_t);
                let icon_color_base = lerp_color(base_icon, colors::FG_PRIMARY, close_t);
                let icon_color = [
                    icon_color_base[0],
                    icon_color_base[1],
                    icon_color_base[2],
                    icon_color_base[3] * close_fade,
                ];
                ui.icon_x(close_rect, s(7.0), icon_t, icon_color);
            }

            // Right separator between tabs — single thin hairline
            // Modern approach: surface color difference provides primary separation,
            // the hairline is just a subtle visual cue that fades on hover.
            if !self.content_sized_tabs && !tab.active && i + 1 < self.tabs.len() {
                let next_active = self.tabs.get(i + 1).map_or(false, |t| t.active);
                if !next_active {
                    let next_hover = self.tab_hover_anim.get(i + 1);
                    let sep_fade = 1.0 - (hover_t.max(next_hover));
                    ui.vline_fade(
                        rect.right(),
                        rect.y + s(8.0),
                        rect.height - s(16.0),
                        1.0,
                        [
                            colors::BORDER[0],
                            colors::BORDER[1],
                            colors::BORDER[2],
                            0.15 * sep_fade,
                        ],
                        s(8.0),
                    );
                }
            }
        }

        // "+ New tab" button after last tab — subtle pill icon button
        if self.show_new_tab_button && layout.new_tab.width > 0.0 {
            let new_t = self.new_tab_anim.value();
            let new_rect = layout.new_tab;
            let btn_radius = new_rect.width / 2.0;
            if new_t > 0.005 {
                // Hover: brightening circular background
                let new_bg = lerp_color(
                    [
                        colors::BG_RAISED[0] * 1.06,
                        colors::BG_RAISED[1] * 1.06,
                        colors::BG_RAISED[2] * 1.06,
                        0.5,
                    ],
                    colors::BG_SURFACE,
                    new_t,
                );
                // Flat hover fill — no gradient, no border
                ui.fill_rounded(new_rect, new_bg, btn_radius);
            } else {
                // Rest: subtle circular border for discoverable icon button
                let rest_border = [
                    colors::BORDER[0],
                    colors::BORDER[1],
                    colors::BORDER[2],
                    0.18,
                ];
                ui.stroke_rounded(new_rect, btn_radius, 0.5, rest_border);
            }
            let new_tab_fg = lerp_color(colors::FG_DIM, colors::FG_SECONDARY, new_t);
            ui.icon_plus(new_rect, icon_t, s(5.0), new_tab_fg);
        }

        // Right-side process indicators — web: display flex, gap 10, paddingRight 14,
        // fontSize 11, color "#555d6b", fontWeight 600
        // Shows active processes: 🟠 bun, ● opensessions
        let buttons = [
            (layout.buttons[0], WindowButton::Minimize),
            (layout.buttons[1], WindowButton::Maximize),
            (layout.buttons[2], WindowButton::Close),
        ];
        if self.show_indicators && layout.indicators.width > 0.0 {
            let indicator_color = colors::FG_DIM; // #555d6b
            let indicator_gap = s(10.0);
            let indicator_pad_r = s(14.0);
            let indicators_rect = layout.indicators;
            let indicators_x_end = indicators_rect.right() - indicator_pad_r;
            let ind_ch = ch * font_scale::PX11;
            let iy = indicators_rect.y + (indicators_rect.height - ind_ch) / 2.0;

            // "opensessions" with green dot — web: fontSize 11, fontWeight 600
            let opensessions_label = "opensessions";
            let opensessions_w = text.text_width_ui_weighted_scaled(opensessions_label, font_scale::PX11, FontWeight::SemiBold);
            let dot_sz = s(8.0);
            let dot_gap = s(4.0);
            let opensessions_total = dot_sz + dot_gap + opensessions_w;
            let opensessions_x = indicators_x_end - opensessions_total;
            // Green dot (8x8, borderRadius 50%, #22c55e)
            let dot_y = indicators_rect.y + (indicators_rect.height - dot_sz) / 2.0;
            ui.fill_rounded(
                Rect {
                    x: opensessions_x,
                    y: dot_y,
                    width: dot_sz,
                    height: dot_sz,
                },
                colors::ACCENT_GREEN,
                dot_sz / 2.0,
            );
            ui.text_ui_semibold_scaled(
                text,
                opensessions_label,
                opensessions_x + dot_sz + dot_gap,
                iy,
                indicator_color,
                colors::BG_RAISED,
                font_scale::PX11,
            );

            // "bun" with orange emoji dot — web: fontSize 11, fontWeight 600
            let bun_label = "bun";
            let bun_w = text.text_width_ui_weighted_scaled(bun_label, font_scale::PX11, FontWeight::SemiBold);
            let bun_dot_sz = s(8.0);
            let bun_total = bun_dot_sz + dot_gap + bun_w;
            let bun_x = opensessions_x - indicator_gap - bun_total;
            // Orange dot (approximating 🟠 emoji with orange circle)
            ui.fill_rounded(
                Rect {
                    x: bun_x,
                    y: dot_y,
                    width: bun_dot_sz,
                    height: bun_dot_sz,
                },
                colors::ACCENT_PEACH,
                bun_dot_sz / 2.0,
            );
            ui.text_ui_semibold_scaled(
                text,
                bun_label,
                bun_x + bun_dot_sz + dot_gap,
                iy,
                indicator_color,
                colors::BG_RAISED,
                font_scale::PX11,
            );
        }

        // Subtle separator before window controls — softer to match quieter aesthetic
        if self.show_window_controls && layout.controls_gap.width > 0.0 {
            let sep_x = layout.controls_gap.x + (layout.controls_gap.width / 2.0);
            let sep_pad = s(8.0);
            ui.vline_fade(
                sep_x,
                bar.y + sep_pad,
                bar.height - sep_pad * 2.0,
                1.0,
                [
                    colors::BORDER[0],
                    colors::BORDER[1],
                    colors::BORDER[2],
                    0.20,
                ],
                s(8.0),
            );
        }
        if self.show_window_controls {
            for (rect, btn) in &buttons {
                let btn_t = match btn {
                    WindowButton::Minimize => self.btn_minimize_anim.value(),
                    WindowButton::Maximize => self.btn_maximize_anim.value(),
                    WindowButton::Close => self.btn_close_anim.value(),
                };

                if btn_t > 0.005 {
                    let base_color = if *btn == WindowButton::Close {
                        [
                            colors::ACCENT_RED[0],
                            colors::ACCENT_RED[1],
                            colors::ACCENT_RED[2],
                            colors::ACCENT_RED[3] * btn_t,
                        ]
                    } else {
                        [
                            colors::BG_HOVER[0],
                            colors::BG_HOVER[1],
                            colors::BG_HOVER[2],
                            colors::BG_HOVER[3] * btn_t,
                        ]
                    };
                    // Flat hover fill — no gradient, no border
                    ui.fill_rounded(*rect, base_color, s(3.0));
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
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect, scale: f32) -> Option<UiAction> {
        let layout = self.layout(bar, scale, None);
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_tab = None;
                self.hovered_close_tab = None;
                self.hovered_button = None;
                self.hovered_new_tab = self.show_new_tab_button && layout.new_tab.contains(x, y);
                let close_btn_sz = 16.0 * scale;
                let close_btn_pad = 8.0 * scale;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let rect = layout.tabs[i];
                    if rect.contains(x, y) {
                        self.hovered_tab = Some(i);
                        // Check close button hover (visible on active + hovered tabs)
                        let show_close = tab.active || true; // hovered_tab is already Some(i)
                        if show_close {
                            if self.show_tab_close_buttons {
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
                }
                if self.show_window_controls {
                    for (rect, btn) in &[
                        (layout.buttons[0], WindowButton::Minimize),
                        (layout.buttons[1], WindowButton::Maximize),
                        (layout.buttons[2], WindowButton::Close),
                    ] {
                        if rect.contains(x, y) {
                            self.hovered_button = Some(*btn);
                        }
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                // Check window buttons first
                if self.show_window_controls {
                    for (rect, btn) in &[
                        (layout.buttons[0], WindowButton::Minimize),
                        (layout.buttons[1], WindowButton::Maximize),
                        (layout.buttons[2], WindowButton::Close),
                    ] {
                        if rect.contains(x, y) {
                            return match btn {
                                WindowButton::Close => Some(UiAction::CloseWindow),
                                WindowButton::Minimize => Some(UiAction::MinimizeWindow),
                                WindowButton::Maximize => Some(UiAction::MaximizeWindow),
                            };
                        }
                    }
                }
                // Check tabs — close button takes priority over tab switch
                let close_btn_sz = 16.0 * scale;
                let close_btn_pad = 8.0 * scale;
                for (i, tab) in self.tabs.iter().enumerate() {
                    let rect = layout.tabs[i];
                    if rect.contains(x, y) {
                        // Check if click landed on the close button area
                        if self.show_tab_close_buttons {
                            let close_rect = Rect {
                                x: rect.right() - close_btn_sz - close_btn_pad,
                                y: rect.y + (rect.height - close_btn_sz) / 2.0,
                                width: close_btn_sz,
                                height: close_btn_sz,
                            };
                            if close_rect.contains(x, y) {
                                return Some(UiAction::CloseTab(tab.id.clone()));
                            }
                        }
                        return Some(UiAction::SwitchTab(tab.id.clone()));
                    }
                }
                if self.show_new_tab_button && layout.new_tab.contains(x, y) {
                    return Some(UiAction::NewTab);
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
