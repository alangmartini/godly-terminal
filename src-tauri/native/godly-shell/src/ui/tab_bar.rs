//! Tab bar: horizontal row of tab buttons.

use super::quad_renderer::{quad_vertices, QuadVertex};
use super::widget::{Rect, UiAction, MouseEvent};

const BG_COLOR: [f32; 4] = [0.09, 0.09, 0.11, 1.0];
const ACTIVE_TAB_COLOR: [f32; 4] = [0.16, 0.16, 0.19, 1.0];
const HOVER_TAB_COLOR: [f32; 4] = [0.13, 0.13, 0.16, 1.0];
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

    pub fn build_quads(&self, bar: Rect, vw: f32, vh: f32) -> Vec<QuadVertex> {
        let mut verts = Vec::new();

        // Background
        verts.extend_from_slice(&quad_vertices(bar.x, bar.y, bar.width, bar.height, vw, vh, BG_COLOR));

        // Tab backgrounds
        for (i, tab) in self.tabs.iter().enumerate() {
            let rect = self.tab_rect(i, bar);
            let color = if tab.active {
                ACTIVE_TAB_COLOR
            } else if self.hovered_tab == Some(i) {
                HOVER_TAB_COLOR
            } else {
                continue; // No background for inactive non-hovered tabs
            };
            verts.extend_from_slice(&quad_vertices(rect.x, rect.y, rect.width, rect.height, vw, vh, color));
        }

        verts
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
