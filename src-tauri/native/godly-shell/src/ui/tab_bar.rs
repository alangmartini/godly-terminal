//! Tab bar: horizontal row of tab buttons with numbered indicators.
//! Also serves as the title bar (window drag + min/max/close buttons).

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const TAB_MAX_WIDTH: f32 = 170.0;
const TAB_MIN_WIDTH: f32 = 90.0;
const TAB_GAP: f32 = 1.0;
const TAB_MARGIN_LEFT: f32 = 6.0;
const TAB_INSET_V: f32 = 5.0;
const RIGHT_INDICATORS_WIDTH: f32 = 200.0;
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
}

impl TabBar {
    pub fn new() -> Self {
        Self { tabs: Vec::new(), hovered_tab: None, hovered_close_tab: None, hovered_button: None, hovered_new_tab: false, sidebar_width: 0.0 }
    }

    /// The x-offset where tabs begin (after sidebar area).
    fn tabs_origin_x(&self, bar: Rect, scale: f32) -> f32 {
        let tab_margin = (TAB_MARGIN_LEFT * scale).round();
        bar.x + self.sidebar_width + tab_margin
    }

    /// Compute the effective tab width that fits all tabs in the available space.
    fn effective_tab_width(&self, bar: Rect, scale: f32) -> f32 {
        let tab_gap = (TAB_GAP * scale).round();
        let right_reserve = (RIGHT_INDICATORS_WIDTH * scale).round();
        let btn_reserve = (BUTTON_WIDTH * scale).round() * 3.0;
        let n = self.tabs.len().max(1) as f32;
        let origin = self.tabs_origin_x(bar, scale);
        let avail = bar.right() - origin - right_reserve - btn_reserve - (40.0 * scale).round();
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
        let tab_inset = (TAB_INSET_V * scale).round();
        let origin = self.tabs_origin_x(bar, scale);
        let new_x = origin + self.tabs.len() as f32 * (tab_w + tab_gap) + (8.0 * scale).round();
        Rect {
            x: new_x,
            y: bar.y + tab_inset,
            width: (28.0 * scale).round(),
            height: bar.height - tab_inset,
        }
    }

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

        // Top edge bevel highlight (very subtle, creates solid edge feel)
        ui.hline(bar.x, bar.y, bar.width, 1.0, [1.0, 1.0, 1.0, 0.03]);

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
                    ui.hline(bar.x, bar.bottom() - 1.0, seg_end - bar.x, 1.0, colors::BORDER);
                }
                // Left ear: small BG_BASE rounded corner that creates concave curve
                // Only bottom-right corner is rounded to form the inverse curve
                let ear_rect = Rect {
                    x: ar.x - ear_r,
                    y: bar.bottom() - ear_r,
                    width: ear_r,
                    height: ear_r,
                };
                ui.fill_rounded_custom(ear_rect, colors::BG_DARK, [0.0, 0.0, ear_r, 0.0]);
            }
            // Right segment (active tab right edge to bar end, shortened for ear)
            if ar.right() < bar.right() {
                let seg_start = ar.right() + ear_r;
                if seg_start < bar.right() {
                    ui.hline(seg_start, bar.bottom() - 1.0, bar.right() - seg_start, 1.0, colors::BORDER);
                }
                // Right ear: concave curve on the other side
                let ear_rect = Rect {
                    x: ar.right(),
                    y: bar.bottom() - ear_r,
                    width: ear_r,
                    height: ear_r,
                };
                ui.fill_rounded_custom(ear_rect, colors::BG_DARK, [0.0, 0.0, 0.0, ear_r]);
            }
        } else {
            ui.hline(bar.x, bar.bottom() - 1.0, bar.width, 1.0, colors::BORDER);
        }

        // Sidebar section: "Godly Terminal" branding
        if self.sidebar_width > 0.0 {
            let brand_x = bar.x + s(12.0);
            let brand_y = text_y(bar.height, bar.y);
            ui.text(text, "Godly Terminal", brand_x, brand_y,
                    colors::FG_SECONDARY, colors::BG_DARK);
            // Right border for sidebar section (faded for softer look)
            ui.vline_fade(self.sidebar_width - 1.0, bar.y, bar.height, 1.0, colors::BORDER, s(8.0));
        }

        // Icon line thickness (used for close buttons in tabs and window controls)
        let icon_t = (ICON_LINE_T * text.scale).max(1.0);

        // Close button dimensions (reserved on right side of every tab)
        let close_btn_sz = s(16.0);
        let close_btn_pad = s(8.0);

        // Max chars for tab title (dynamic based on effective width, reserves close button space)
        let title_x_offset = cw * 2.0 + s(22.0);
        let title_max_w = tab_w - title_x_offset - close_btn_sz - close_btn_pad - s(4.0);
        let title_max_chars = (title_max_w / cw).floor().max(1.0) as usize;

        for (i, tab) in self.tabs.iter().enumerate() {
            let rect = Rect {
                x: origin + i as f32 * (tab_w + tab_gap),
                y: bar.y + tab_inset,
                width: tab_w,
                height: bar.height - tab_inset,
            };
            let accent = self.accent_for(i);

            // Tab background — active tabs match terminal bg, inactive are subtle
            let bg = if tab.active {
                colors::BG_BASE
            } else if self.hovered_tab == Some(i) {
                colors::BG_SURFACE
            } else {
                colors::BG_DARK
            };

            let tab_radius = s(5.0);
            if tab.active {
                // Active tab: top-only rounded gradient (slightly lighter top → BG_BASE bottom)
                let tab_top = [
                    bg[0] * 1.12,
                    bg[1] * 1.12,
                    bg[2] * 1.12,
                    1.0,
                ];
                ui.fill_rounded_top_gradient(rect, tab_top, bg, tab_radius, 1.0, colors::BORDER);
                // Full-tab ambient glow from accent color (subtle luminosity across entire tab)
                let ambient_rect = Rect {
                    x: rect.x + 1.0,
                    y: rect.y + 1.0,
                    width: rect.width - 2.0,
                    height: rect.height - 1.0,
                };
                let ambient_top = [accent[0], accent[1], accent[2], 0.06];
                let ambient_bottom = [accent[0], accent[1], accent[2], 0.0];
                ui.fill_gradient(ambient_rect, ambient_top, ambient_bottom);
                // Soft glow behind the accent bar (luminous highlight)
                let glow_rect = Rect {
                    x: rect.x + tab_radius,
                    y: rect.y,
                    width: rect.width - tab_radius * 2.0,
                    height: s(6.0),
                };
                let glow_color = [accent[0], accent[1], accent[2], 0.12];
                ui.fill_rounded(glow_rect, glow_color, s(3.0));
                // Top accent bar (inset from corners)
                ui.hline(rect.x + tab_radius, rect.y + 1.0, rect.width - tab_radius * 2.0, s(2.0), accent);
                // Subtle inner highlight just below border (bevel effect)
                ui.hline(rect.x + tab_radius + 1.0, rect.y + s(2.0) + 1.0,
                         rect.width - (tab_radius + 1.0) * 2.0, 1.0,
                         [1.0, 1.0, 1.0, 0.04]);
            } else if self.hovered_tab == Some(i) {
                // Hovered tab: top-only rounding with gradient + border
                let hover_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.6];
                let hover_top = [bg[0] * 1.06, bg[1] * 1.06, bg[2] * 1.06, 1.0];
                ui.fill_rounded_top_gradient(rect, hover_top, bg, s(4.0), 0.5, hover_border);
            } else {
                // Inactive tab: very subtle background tint to show it's interactive
                let inactive_bg = [
                    colors::BG_DARK[0] * 1.04,
                    colors::BG_DARK[1] * 1.04,
                    colors::BG_DARK[2] * 1.04,
                    0.5,
                ];
                ui.fill_rounded_top(rect, inactive_bg, s(3.0));
            }

            // Colored dot indicator (circle via SDF + subtle glow)
            let dot_x = rect.x + s(10.0);
            let dot_sz = s(5.0);
            let dot_y = rect.y + rect.height / 2.0 - dot_sz / 2.0;
            // Soft glow behind the dot for luminosity
            let dot_glow_rect = Rect {
                x: dot_x - s(2.0), y: dot_y - s(2.0),
                width: dot_sz + s(4.0), height: dot_sz + s(4.0),
            };
            ui.fill_shadow(dot_glow_rect, [accent[0], accent[1], accent[2], 0.15], dot_sz, s(4.0));
            ui.fill_rounded(
                Rect { x: dot_x, y: dot_y, width: dot_sz, height: dot_sz },
                accent,
                dot_sz / 2.0,
            );

            // Tab number (after dot with spacing)
            let num_str = format!("{}", i + 1);
            let num_x = dot_x + dot_sz + s(5.0);
            ui.text(text, &num_str,
                    num_x,
                    text_y(rect.height, rect.y),
                    accent, bg);

            // Tab title (truncated to fit)
            let fg = if tab.active { colors::FG_PRIMARY } else { colors::FG_SECONDARY };
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
                let title_x = num_x + cw + s(4.0);
                ui.text(text, &title,
                        title_x,
                        text_y(rect.height, rect.y),
                        fg, bg);
            }

            // Close button — visible on active tab always, and on hovered tabs
            let show_close = tab.active || self.hovered_tab == Some(i);
            if show_close {
                let close_rect = Rect {
                    x: rect.right() - close_btn_sz - close_btn_pad,
                    y: rect.y + (rect.height - close_btn_sz) / 2.0,
                    width: close_btn_sz,
                    height: close_btn_sz,
                };
                // Hover circle behind X icon (VS Code style, gradient for depth)
                let close_hovered = self.hovered_close_tab == Some(i);
                if close_hovered {
                    let hover_top = [
                        colors::BG_HOVER[0] * 1.1,
                        colors::BG_HOVER[1] * 1.1,
                        colors::BG_HOVER[2] * 1.1,
                        colors::BG_HOVER[3],
                    ];
                    ui.fill_rounded_gradient(close_rect, hover_top, colors::BG_HOVER, close_btn_sz / 2.0);
                }
                let icon_color = if close_hovered {
                    colors::FG_PRIMARY
                } else if tab.active {
                    colors::FG_SECONDARY
                } else {
                    colors::FG_MUTED
                };
                let icon_sz = s(7.0);
                ui.icon_x(close_rect, icon_sz, icon_t, icon_color);
            }

            // Right separator between tabs (faded, skip for active and last tab)
            if !tab.active && i + 1 < self.tabs.len() {
                let next_active = self.tabs.get(i + 1).map_or(false, |t| t.active);
                if !next_active {
                    let sep_color = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5];
                    ui.vline_fade(rect.right(), rect.y + s(6.0), rect.height - s(12.0), 1.0, sep_color, s(6.0));
                }
            }
        }

        // "+ New tab" button after last tab
        let new_x = origin + self.tabs.len() as f32 * (tab_w + tab_gap) + s(8.0);
        let new_y = bar.y + tab_inset;
        let new_rect = Rect { x: new_x, y: new_y, width: s(28.0), height: bar.height - tab_inset };
        let new_tab_bg = if self.hovered_new_tab {
            let new_top = [
                colors::BG_SURFACE[0] * 1.08,
                colors::BG_SURFACE[1] * 1.08,
                colors::BG_SURFACE[2] * 1.08,
                colors::BG_SURFACE[3],
            ];
            ui.fill_rounded_top_gradient(new_rect, new_top, colors::BG_SURFACE, s(4.0), 0.5, colors::BORDER);
            colors::BG_SURFACE
        } else {
            // Subtle rest-state hint: faint rounded rect to signal interactivity
            let rest_bg = [colors::BG_DARK[0] * 1.06, colors::BG_DARK[1] * 1.06, colors::BG_DARK[2] * 1.06, 0.4];
            ui.fill_rounded_top(new_rect, rest_bg, s(3.0));
            colors::BG_DARK
        };
        let new_tab_fg = if self.hovered_new_tab { colors::FG_SECONDARY } else { colors::FG_MUTED };
        ui.text(text, "+", new_x + s(8.0), text_y(new_rect.height, new_y), new_tab_fg, new_tab_bg);

        // Right-aligned indicators (bun icon + session name) — positioned before window buttons
        let btn_reserve = s(BUTTON_WIDTH) * 3.0 + s(8.0);
        let right_label = "bun";
        let right_label2 = "opensessions";
        let rw2 = text.text_width(right_label2);
        let rw1 = text.text_width(right_label);
        let gap = cw * 2.0;
        // Session name
        ui.text(text, right_label2,
                bar.right() - rw2 - btn_reserve,
                text_y(bar.height, bar.y),
                colors::FG_MUTED, colors::BG_DARK);
        // Bun indicator with accent color
        let bun_dot_x = bar.right() - rw2 - btn_reserve - gap - rw1 - s(10.0);
        let dot_sz = s(5.0);
        ui.fill_rounded(
            Rect { x: bun_dot_x, y: bar.y + bar.height / 2.0 - dot_sz / 2.0, width: dot_sz, height: dot_sz },
            colors::ACCENT_PEACH,
            dot_sz / 2.0,
        );
        ui.text(text, right_label,
                bun_dot_x + s(10.0),
                text_y(bar.height, bar.y),
                colors::FG_SECONDARY, colors::BG_DARK);

        // Window control buttons (minimize, maximize, close)
        let buttons = self.window_button_rects(bar, text.scale);
        for (rect, btn) in &buttons {
            let hovered = self.hovered_button == Some(*btn);
            if hovered {
                let color = if *btn == WindowButton::Close {
                    colors::ACCENT_RED
                } else {
                    colors::BG_HOVER
                };
                ui.fill_rounded(*rect, color, s(3.0));
            }

            let icon_color = if hovered && *btn == WindowButton::Close {
                colors::WHITE
            } else {
                colors::FG_MUTED
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
