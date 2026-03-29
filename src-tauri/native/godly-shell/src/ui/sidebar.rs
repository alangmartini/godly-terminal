//! Left sidebar: workspace icons, settings, new workspace.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const ICON_SIZE: f32 = 24.0;
const ICON_PADDING: f32 = 12.0;
const SETTINGS_Y_OFFSET: f32 = 8.0;

pub struct SidebarItem {
    pub id: String,
    pub label: char,
    pub active: bool,
}

pub struct Sidebar {
    pub items: Vec<SidebarItem>,
    pub hovered_index: Option<usize>,
    pub hovered_settings: bool,
    pub hovered_new: bool,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            items: vec![SidebarItem { id: "default".into(), label: '1', active: true }],
            hovered_index: None,
            hovered_settings: false,
            hovered_new: false,
        }
    }

    fn item_rect(&self, index: usize, sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x + (sidebar.width - ICON_SIZE) / 2.0,
            y: sidebar.y + ICON_PADDING + index as f32 * (ICON_SIZE + ICON_PADDING),
            width: ICON_SIZE,
            height: ICON_SIZE,
        }
    }

    fn new_workspace_rect(&self, sidebar: Rect) -> Rect {
        let y = sidebar.y + ICON_PADDING + self.items.len() as f32 * (ICON_SIZE + ICON_PADDING);
        Rect {
            x: sidebar.x + (sidebar.width - ICON_SIZE) / 2.0,
            y,
            width: ICON_SIZE,
            height: ICON_SIZE,
        }
    }

    fn settings_rect(&self, sidebar: Rect) -> Rect {
        Rect {
            x: sidebar.x + (sidebar.width - ICON_SIZE) / 2.0,
            y: sidebar.y + sidebar.height - ICON_SIZE - SETTINGS_Y_OFFSET,
            width: ICON_SIZE,
            height: ICON_SIZE,
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, sidebar: Rect, text: &UiTextRenderer) {
        if sidebar.width < 1.0 { return; }

        ui.fill(sidebar, colors::BG_DARK);

        // Workspace icons
        for (i, item) in self.items.iter().enumerate() {
            let rect = self.item_rect(i, sidebar);

            if self.hovered_index == Some(i) {
                ui.fill(rect.expand(2.0), colors::BG_SURFACE);
            }
            if item.active {
                ui.fill(sidebar.sub(0.0, rect.y - sidebar.y, 3.0, rect.height), colors::ACCENT_BLUE);
            }

            let icon_bg = if item.active { colors::BG_ACTIVE_ACC } else { colors::BG_SURFACE };
            ui.fill(rect, icon_bg);

            // Workspace number
            ui.text(text, &item.label.to_string(), rect.x + 7.0, rect.y + 4.0, colors::FG_PRIMARY, colors::TRANSPARENT);
        }

        // "+" new workspace button
        let new_rect = self.new_workspace_rect(sidebar);
        if self.hovered_new { ui.fill(new_rect.expand(2.0), colors::BG_SURFACE); }
        ui.icon_plus(new_rect, 2.0, 6.0, colors::FG_MUTED);

        // Settings gear (bottom)
        let gear_rect = self.settings_rect(sidebar);
        if self.hovered_settings { ui.fill(gear_rect.expand(2.0), colors::BG_SURFACE); }
        ui.icon_gear(gear_rect, 20.0, 12.0, colors::FG_MUTED, colors::BG_DARK);
    }

    pub fn on_mouse(&mut self, event: MouseEvent, sidebar: Rect) -> Option<UiAction> {
        if sidebar.width < 1.0 { return None; }
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_index = None;
                self.hovered_new = false;
                self.hovered_settings = false;
                for (i, _) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar).contains(x, y) { self.hovered_index = Some(i); }
                }
                if self.new_workspace_rect(sidebar).contains(x, y) { self.hovered_new = true; }
                if self.settings_rect(sidebar).contains(x, y) { self.hovered_settings = true; }
                None
            }
            MouseEvent::Press { x, y } => {
                for (i, item) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar).contains(x, y) {
                        return Some(UiAction::SwitchTab(item.id.clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
