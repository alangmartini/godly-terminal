//! Title bar: drag-to-move, window title, min/max/close buttons.
//! Uses Catppuccin Mocha palette, blends with tab bar below.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const BUTTON_WIDTH: f32 = 46.0;
const ICON_LINE_T: f32 = 1.2;

pub struct TitleBar {
    pub hovered_button: Option<TitleButton>,
    pub sidebar_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitleButton {
    Minimize,
    Maximize,
    Close,
}

impl TitleBar {
    pub fn new() -> Self {
        Self { hovered_button: None, sidebar_width: 0.0 }
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

        // Bottom separator
        ui.hline(bar.x, bar.bottom() - 1.0, bar.width, 1.0, colors::BORDER);

        // Sidebar section border (consistent with status bar)
        if self.sidebar_width > 0.0 {
            ui.vline(self.sidebar_width - 1.0, bar.y, bar.height, 1.0, colors::BORDER);
        }

        // App title text (left-aligned in sidebar section)
        let title_str = "Godly Terminal";
        let title_x = bar.x + s(12.0);
        let title_y = bar.y + (bar.height - ch) / 2.0;
        ui.text(text, title_str, title_x, title_y, colors::FG_SECONDARY, colors::BG_DARK);

        // Button hover highlights and icons
        let icon_t = (ICON_LINE_T * text.scale).max(1.0);
        let buttons = self.scaled_button_rects(bar, text.scale);
        for (rect, btn) in &buttons {
            let hovered = self.hovered_button == Some(*btn);
            if hovered {
                let color = if *btn == TitleButton::Close {
                    colors::ACCENT_RED
                } else {
                    colors::BG_HOVER
                };
                ui.fill_rounded(*rect, color, s(3.0));
            }

            let icon_color = if hovered && *btn == TitleButton::Close {
                colors::WHITE
            } else {
                colors::FG_MUTED
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
