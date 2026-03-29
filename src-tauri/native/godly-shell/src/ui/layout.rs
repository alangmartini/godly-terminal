//! Layout constants and computation for the shell chrome.

/// Heights for UI chrome elements (in pixels).
pub const TITLE_BAR_HEIGHT: f32 = 32.0;
pub const TAB_BAR_HEIGHT: f32 = 36.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Computed layout rectangles for the shell regions.
#[derive(Debug, Clone, Copy)]
pub struct ShellLayout {
    pub title_bar: super::widget::Rect,
    pub tab_bar: super::widget::Rect,
    pub terminal: super::widget::Rect,
    pub status_bar: super::widget::Rect,
}

impl ShellLayout {
    pub fn compute(viewport_w: f32, viewport_h: f32) -> Self {
        let title_bar = super::widget::Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_w,
            height: TITLE_BAR_HEIGHT,
        };
        let tab_bar = super::widget::Rect {
            x: 0.0,
            y: TITLE_BAR_HEIGHT,
            width: viewport_w,
            height: TAB_BAR_HEIGHT,
        };
        let terminal_y = TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
        let terminal_h = (viewport_h - terminal_y - STATUS_BAR_HEIGHT).max(0.0);
        let terminal = super::widget::Rect {
            x: 0.0,
            y: terminal_y,
            width: viewport_w,
            height: terminal_h,
        };
        let status_bar = super::widget::Rect {
            x: 0.0,
            y: terminal_y + terminal_h,
            width: viewport_w,
            height: STATUS_BAR_HEIGHT,
        };
        Self { title_bar, tab_bar, terminal, status_bar }
    }
}
