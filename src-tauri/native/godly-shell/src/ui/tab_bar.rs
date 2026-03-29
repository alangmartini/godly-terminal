//! Tab bar: horizontal row of tab buttons.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const TAB_WIDTH: f32 = 160.0;
const TAB_PADDING: f32 = 4.0;

pub struct TabInfo {
    pub id: String,
    pub title: String,
    pub active: bool,
}

pub struct TabBar {
    pub tabs: Vec<TabInfo>,
    pub hovered_tab: Option<usize>,
}

impl TabBar {
    pub fn new() -> Self {
        Self { tabs: Vec::new(), hovered_tab: None }
    }

    fn tab_rect(&self, index: usize, bar: Rect) -> Rect {
        Rect {
            x: bar.x + TAB_PADDING + index as f32 * (TAB_WIDTH + TAB_PADDING),
            y: bar.y + TAB_PADDING,
            width: TAB_WIDTH,
            height: bar.height - TAB_PADDING * 2.0,
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, bar: Rect, text: &UiTextRenderer) {
        // Background
        ui.fill(bar, colors::BG_BASE);

        // Tab backgrounds and labels
        for (i, tab) in self.tabs.iter().enumerate() {
            let rect = self.tab_rect(i, bar);
            let color = if tab.active {
                colors::BG_ACTIVE
            } else if self.hovered_tab == Some(i) {
                colors::BG_RAISED
            } else {
                colors::BG_BASE
            };
            ui.fill(rect, color);

            // Active tab accent indicator (bottom edge)
            if tab.active {
                ui.hline(rect.x, rect.bottom() - 2.0, rect.width, 2.0, colors::ACCENT_BLUE);
            }

            // Tab title text (truncate to 20 chars with ellipsis)
            let fg = if tab.active { colors::FG_PRIMARY } else { colors::FG_SECONDARY };
            if tab.title.len() > 20 {
                let truncated = format!("{}\u{2026}", &tab.title[..19]);
                ui.text(text, &truncated, rect.x + 8.0, rect.y + 4.0, fg, colors::TRANSPARENT);
            } else {
                ui.text(text, &tab.title, rect.x + 8.0, rect.y + 4.0, fg, colors::TRANSPARENT);
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, bar: Rect) -> Option<UiAction> {
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_tab = None;
                for (i, _) in self.tabs.iter().enumerate() {
                    if self.tab_rect(i, bar).contains(x, y) {
                        self.hovered_tab = Some(i);
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                for (i, tab) in self.tabs.iter().enumerate() {
                    if self.tab_rect(i, bar).contains(x, y) {
                        return Some(UiAction::SwitchTab(tab.id.clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
