//! Right panel: contextual detail/content panel.
//!
//! Follows the web reference layout: header with title + close button,
//! scrollable content area, and a small status bar at the bottom.

use super::anim::{self, Anim, lerp_color};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, MouseEvent};

pub struct RightPanel {
    /// Whether the right panel is currently shown.
    pub visible: bool,
    /// Header title text.
    pub title: String,
    /// Content lines to display in the panel body.
    pub content_lines: Vec<String>,
    // Close button hover animation
    close_hover_anim: Anim,
    close_hovered: bool,
}

impl RightPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            title: String::new(),
            content_lines: Vec::new(),
            close_hover_anim: Anim::default(),
            close_hovered: false,
        }
    }

    /// Advance hover animations. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.close_hover_anim.set(if self.close_hovered { 1.0 } else { 0.0 });
        self.close_hover_anim.tick(hl, dt)
    }

    pub fn build(&self, ui: &mut UiBuilder, panel: Rect, status: Rect, text: &UiTextRenderer) {
        if !self.visible || panel.width < 1.0 {
            return;
        }

        let s = |v: f32| text.s(v);
        let ch = text.cell_height;

        // Panel background
        ui.fill(panel, colors::BG_DARK);

        // Left border
        ui.vline(panel.x, panel.y, panel.height, 1.0,
            [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.35]);

        // --- Header ---
        let header_h = s(36.0);
        let header = Rect { x: panel.x, y: panel.y, width: panel.width, height: header_h };

        // Header bottom border
        ui.hline(header.x, header.bottom() - 1.0, header.width, 1.0,
            [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.25]);

        // Title text
        let title_y = header.y + (header_h - ch) / 2.0;
        let title = if self.title.is_empty() { "Panel" } else { &self.title };
        ui.text_ui(text, title, panel.x + s(14.0), title_y, colors::FG_PRIMARY, colors::BG_DARK);

        // Close button (x)
        let close_sz = ch;
        let close_x = panel.right() - close_sz - s(10.0);
        let close_y = header.y + (header_h - close_sz) / 2.0;
        let close_rect = Rect { x: close_x, y: close_y, width: close_sz, height: close_sz };
        let close_t = self.close_hover_anim.value();
        let close_fg = lerp_color(colors::FG_MUTED, colors::FG_PRIMARY, close_t);
        let icon_t = (0.8 * text.scale).max(1.0);
        ui.icon_x(close_rect, s(7.0), icon_t, close_fg);

        // --- Content area ---
        let content_y = header.bottom();
        let content_h = if status.height > 0.0 {
            (status.y - content_y).max(0.0)
        } else {
            (panel.bottom() - content_y).max(0.0)
        };
        let content_rect = Rect {
            x: panel.x + s(16.0),
            y: content_y + s(12.0),
            width: panel.width - s(32.0),
            height: content_h - s(12.0),
        };

        // Render content lines (simple text, no scroll yet)
        let line_height = ch * 1.6;
        let mut y = content_rect.y;
        for line in &self.content_lines {
            if y + ch > content_rect.y + content_rect.height {
                break; // clip
            }
            let fg = colors::FG_SECONDARY;
            ui.text_ui(text, line, content_rect.x, y, fg, colors::BG_DARK);
            y += line_height;
        }

        // --- Bottom status bar ---
        if status.height > 0.0 {
            ui.fill(status, colors::BG_STATUS);
            ui.hline(status.x, status.y, status.width, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.25]);

            let status_y = status.y + (status.height - ch) / 2.0;
            // Left: "}" brace
            ui.text_ui(text, "}", status.x + s(10.0), status_y, colors::FG_MUTED, colors::BG_STATUS);
            // Right: "? for shortcuts"
            let hint = "? for shortcuts";
            let hint_w = text.text_width_ui(hint);
            ui.text_ui(text, hint, status.right() - hint_w - s(10.0), status_y, colors::FG_MUTED, colors::BG_STATUS);
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, panel: Rect, text: &UiTextRenderer) -> Option<RightPanelAction> {
        if !self.visible || panel.width < 1.0 {
            return None;
        }

        let s = |v: f32| text.s(v);
        let ch = text.cell_height;
        let header_h = s(36.0);

        // Close button hit rect
        let close_sz = ch;
        let close_x = panel.right() - close_sz - s(10.0);
        let close_y = panel.y + (header_h - close_sz) / 2.0;
        let close_rect = Rect { x: close_x, y: close_y, width: close_sz, height: close_sz };

        match event {
            MouseEvent::Move { x, y } => {
                self.close_hovered = close_rect.contains(x, y);
                None
            }
            MouseEvent::Press { x, y, .. } => {
                if close_rect.contains(x, y) {
                    Some(RightPanelAction::Close)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RightPanelAction {
    Close,
}
