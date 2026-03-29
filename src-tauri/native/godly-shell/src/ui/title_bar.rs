//! Title bar: drag-to-move, window title, min/max/close buttons.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const BUTTON_WIDTH: f32 = 46.0;
const ICON_LINE_T: f32 = 1.2;

pub struct TitleBar {
    pub hovered_button: Option<TitleButton>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitleButton {
    Minimize,
    Maximize,
    Close,
}

impl TitleBar {
    pub fn new() -> Self {
        Self { hovered_button: None }
    }

    fn button_rects(&self, bar: Rect) -> [(Rect, TitleButton); 3] {
        let close = Rect {
            x: bar.x + bar.width - BUTTON_WIDTH,
            y: bar.y,
            width: BUTTON_WIDTH,
            height: bar.height,
        };
        let maximize = Rect {
            x: close.x - BUTTON_WIDTH,
            y: bar.y,
            width: BUTTON_WIDTH,
            height: bar.height,
        };
        let minimize = Rect {
            x: maximize.x - BUTTON_WIDTH,
            y: bar.y,
            width: BUTTON_WIDTH,
            height: bar.height,
        };
        [
            (minimize, TitleButton::Minimize),
            (maximize, TitleButton::Maximize),
            (close, TitleButton::Close),
        ]
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        // Title bar background
        ui.fill(bar, colors::BG_RAISED);

        // Button hover highlights and icons
        for (rect, btn) in &self.button_rects(bar) {
            let hovered = self.hovered_button == Some(*btn);
            if hovered {
                let color = if *btn == TitleButton::Close {
                    colors::ACCENT_RED
                } else {
                    colors::BG_HOVER
                };
                ui.fill(*rect, color);
            }

            let icon_color = if hovered && *btn == TitleButton::Close {
                colors::WHITE
            } else {
                colors::FG_SECONDARY
            };
            match btn {
                TitleButton::Minimize => ui.icon_minimize(*rect, 10.0, ICON_LINE_T, icon_color),
                TitleButton::Maximize => ui.icon_maximize(*rect, 9.0, ICON_LINE_T, icon_color),
                TitleButton::Close => ui.icon_x(*rect, 9.0, ICON_LINE_T, icon_color),
            }
        }

        // Title text
        ui.text(
            text,
            "Godly Terminal",
            bar.x + 12.0,
            bar.y + (bar.height - 14.0) / 2.0,
            colors::FG_SECONDARY,
            colors::TRANSPARENT,
        );
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect) -> Option<UiAction> {
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_button = None;
                for (rect, btn) in &self.button_rects(bar) {
                    if rect.contains(x, y) {
                        self.hovered_button = Some(*btn);
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                for (rect, btn) in &self.button_rects(bar) {
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
