//! Layout constants and computation for the shell chrome.

use super::widget::Rect;

pub const TITLE_BAR_HEIGHT: f32 = 32.0;
pub const TAB_BAR_HEIGHT: f32 = 36.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
pub const SIDEBAR_WIDTH: f32 = 48.0;

/// Computed layout rectangles for the shell regions.
#[derive(Debug, Clone, Copy)]
pub struct ShellLayout {
    pub title_bar: Rect,
    pub sidebar: Rect,
    pub tab_bar: Rect,
    pub terminal: Rect,
    pub status_bar: Rect,
}

impl ShellLayout {
    pub fn compute(viewport_w: f32, viewport_h: f32, sidebar_visible: bool) -> Self {
        let sidebar_w = if sidebar_visible { SIDEBAR_WIDTH } else { 0.0 };

        let title_bar = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_w,
            height: TITLE_BAR_HEIGHT,
        };
        let sidebar = Rect {
            x: 0.0,
            y: TITLE_BAR_HEIGHT,
            width: sidebar_w,
            height: viewport_h - TITLE_BAR_HEIGHT,
        };
        let content_x = sidebar_w;
        let content_w = viewport_w - sidebar_w;
        let tab_bar = Rect {
            x: content_x,
            y: TITLE_BAR_HEIGHT,
            width: content_w,
            height: TAB_BAR_HEIGHT,
        };
        let terminal_y = TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
        let terminal_h = (viewport_h - terminal_y - STATUS_BAR_HEIGHT).max(0.0);
        let terminal = Rect {
            x: content_x,
            y: terminal_y,
            width: content_w,
            height: terminal_h,
        };
        let status_bar = Rect {
            x: content_x,
            y: terminal_y + terminal_h,
            width: content_w,
            height: STATUS_BAR_HEIGHT,
        };
        Self { title_bar, sidebar, tab_bar, terminal, status_bar }
    }
}
