//! Title bar: drag-to-move, window title, min/max/close buttons.

use super::quad_renderer::{quad_vertices, QuadVertex};
use super::widget::{Rect, UiAction, MouseEvent};

const BG_COLOR: [f32; 4] = [0.12, 0.12, 0.14, 1.0];
const CLOSE_HOVER: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
const BUTTON_HOVER: [f32; 4] = [0.25, 0.25, 0.28, 1.0];
const BUTTON_WIDTH: f32 = 46.0;

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

    pub fn build_quads(&self, bar: Rect, vw: f32, vh: f32) -> Vec<QuadVertex> {
        let mut verts = Vec::new();

        // Title bar background
        verts.extend_from_slice(&quad_vertices(bar.x, bar.y, bar.width, bar.height, vw, vh, BG_COLOR));

        // Button hover highlights
        for (rect, btn) in &self.button_rects(bar) {
            if self.hovered_button == Some(*btn) {
                let color = if *btn == TitleButton::Close { CLOSE_HOVER } else { BUTTON_HOVER };
                verts.extend_from_slice(&quad_vertices(rect.x, rect.y, rect.width, rect.height, vw, vh, color));
            }
        }

        verts
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
