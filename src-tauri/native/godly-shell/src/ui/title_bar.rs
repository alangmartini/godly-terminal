//! Title bar: drag-to-move, window title, min/max/close buttons.
//! Uses Catppuccin Mocha palette, blends with tab bar below.

use super::anim::{self, Anim, lerp_color};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const BUTTON_WIDTH: f32 = 46.0;
const ICON_LINE_T: f32 = 1.2;

pub struct TitleBar {
    pub hovered_button: Option<TitleButton>,
    pub sidebar_width: f32,
    btn_minimize_anim: Anim,
    btn_maximize_anim: Anim,
    btn_close_anim: Anim,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitleButton {
    Minimize,
    Maximize,
    Close,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            hovered_button: None,
            sidebar_width: 0.0,
            btn_minimize_anim: Anim::default(),
            btn_maximize_anim: Anim::default(),
            btn_close_anim: Anim::default(),
        }
    }

    /// Advance hover animations. `dt` = seconds since last frame. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.btn_minimize_anim.set(if self.hovered_button == Some(TitleButton::Minimize) { 1.0 } else { 0.0 });
        self.btn_maximize_anim.set(if self.hovered_button == Some(TitleButton::Maximize) { 1.0 } else { 0.0 });
        self.btn_close_anim.set(if self.hovered_button == Some(TitleButton::Close) { 1.0 } else { 0.0 });
        let mut a = false;
        a |= self.btn_minimize_anim.tick(hl, dt);
        a |= self.btn_maximize_anim.tick(hl, dt);
        a |= self.btn_close_anim.tick(hl, dt);
        a
    }

    fn button_rects(&self, bar: Rect) -> [(Rect, TitleButton); 3] {
        self.scaled_button_rects(bar, 1.0)
    }

    fn scaled_button_rects(&self, bar: Rect, scale: f32) -> [(Rect, TitleButton); 3] {
        let btn_w = (BUTTON_WIDTH * scale).round();
        let close = Rect {
            x: bar.x + bar.width - btn_w,
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
            (minimize, TitleButton::Minimize),
            (maximize, TitleButton::Maximize),
            (close, TitleButton::Close),
        ]
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        let s = |v: f32| text.s(v);
        let ch = text.cell_height;

        // Title bar background — subtle gradient for depth
        let bar_bottom = [
            colors::BG_DARK[0] * 0.92,
            colors::BG_DARK[1] * 0.92,
            colors::BG_DARK[2] * 0.92,
            1.0,
        ];
        ui.fill_gradient(bar, colors::BG_DARK, bar_bottom);

        // Top edge bevel highlight (creates solid window edge feel)
        ui.hline(bar.x, bar.y, bar.width, 1.0, [1.0, 1.0, 1.0, 0.05]);

        // Bottom separator (SDF anti-aliased for crisp edges at any DPI)
        ui.hline_aa(bar.x, bar.bottom() - 1.0, bar.width, 1.0, colors::BORDER);

        // Sidebar section border — groove for embossed depth
        if self.sidebar_width > 0.0 {
            ui.vgroove_fade(self.sidebar_width - 2.0, bar.y, bar.height,
                [0.0, 0.0, 0.0, 0.15], [1.0, 1.0, 1.0, 0.04], s(8.0));
        }

        // App title text (left-aligned in sidebar section)
        let title_str = "Godly Terminal";
        let title_x = bar.x + s(12.0);
        let title_y = bar.y + (bar.height - ch) / 2.0;
        ui.text(text, title_str, title_x, title_y, colors::FG_SECONDARY, colors::BG_DARK);

        // Button hover highlights and icons (animated)
        let icon_t = (ICON_LINE_T * text.scale).max(1.0);
        let buttons = self.scaled_button_rects(bar, text.scale);
        for (rect, btn) in &buttons {
            let btn_t = match btn {
                TitleButton::Minimize => self.btn_minimize_anim.value(),
                TitleButton::Maximize => self.btn_maximize_anim.value(),
                TitleButton::Close => self.btn_close_anim.value(),
            };

            if btn_t > 0.005 {
                let (base_color, border_color) = if *btn == TitleButton::Close {
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

            let icon_color = if *btn == TitleButton::Close {
                lerp_color(colors::FG_MUTED, colors::WHITE, btn_t)
            } else {
                lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, btn_t)
            };
            match btn {
                TitleButton::Minimize => ui.icon_minimize(*rect, s(10.0), icon_t, icon_color),
                TitleButton::Maximize => ui.icon_maximize(*rect, s(9.0), icon_t, icon_color),
                TitleButton::Close => ui.icon_x(*rect, s(9.0), icon_t, icon_color),
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect, scale: f32) -> Option<UiAction> {
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_button = None;
                for (rect, btn) in &self.scaled_button_rects(bar, scale) {
                    if rect.contains(x, y) {
                        self.hovered_button = Some(*btn);
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                for (rect, btn) in &self.scaled_button_rects(bar, scale) {
                    if rect.contains(x, y) {
                        return match btn {
                            TitleButton::Close => Some(UiAction::CloseWindow),
                            TitleButton::Minimize => Some(UiAction::MinimizeWindow),
                            TitleButton::Maximize => Some(UiAction::MaximizeWindow),
                        };
                    }
                }
                // Click on title bar body = drag
                if bar.contains(x, y) {
                    Some(UiAction::DragWindow)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
