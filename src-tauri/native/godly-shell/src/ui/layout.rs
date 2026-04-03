//! Layout constants and computation for the shell chrome.

use super::widget::Rect;

pub const TAB_BAR_HEIGHT: f32 = 36.0;
pub const STATUS_BAR_HEIGHT: f32 = 26.0;
pub const BREADCRUMB_HEIGHT: f32 = 0.0;
pub const SIDEBAR_WIDTH: f32 = 200.0;
pub const RIGHT_PANEL_WIDTH: f32 = 380.0;
pub const TERMINAL_PAD_LEFT: f32 = 8.0;
pub const TERMINAL_PAD_TOP: f32 = 6.0;

/// Computed layout rectangles for the shell regions.
#[derive(Debug, Clone, Copy)]
pub struct ShellLayout {
    pub sidebar: Rect,
    pub tab_bar: Rect,
    /// Breadcrumb/path bar between tab bar and content (content-area width only).
    pub breadcrumb: Rect,
    pub terminal: Rect,
    /// Terminal content area (inset by padding from terminal rect)
    pub terminal_content: Rect,
    pub status_bar: Rect,
    /// Right panel (contextual detail panel). Zero-sized when hidden.
    pub right_panel: Rect,
    /// Status bar at bottom of right panel. Zero-sized when hidden.
    pub right_panel_status: Rect,
}

impl ShellLayout {
    /// Compute layout regions. `scale` is the DPI scale factor (e.g. 1.5).
    /// Layout constants are defined in logical pixels and scaled up for physical rendering.
    pub fn compute(
        viewport_w: f32,
        viewport_h: f32,
        sidebar_visible: bool,
        right_panel_visible: bool,
        sidebar_width: f32,
        right_panel_width: f32,
        scale: f32,
    ) -> Self {
        let tab_h = (TAB_BAR_HEIGHT * scale).round();
        let status_h = (STATUS_BAR_HEIGHT * scale).round();
        let breadcrumb_h = (BREADCRUMB_HEIGHT * scale).round();
        let sidebar_w = if sidebar_visible { (sidebar_width * scale).round() } else { 0.0 };
        let right_w = if right_panel_visible { (right_panel_width * scale).round() } else { 0.0 };

        // Status bar spans full width at the very bottom
        let status_bar = Rect {
            x: 0.0,
            y: viewport_h - status_h,
            width: viewport_w,
            height: status_h,
        };

        // Sidebar starts below the tab bar and stops above the status bar
        let sidebar = Rect {
            x: 0.0,
            y: tab_h,
            width: sidebar_w,
            height: (viewport_h - tab_h - status_h).max(0.0),
        };

        // Right panel occupies right edge, below tab bar, above status bar
        let content_height = (viewport_h - tab_h - status_h).max(0.0);
        let right_panel = Rect {
            x: viewport_w - right_w,
            y: tab_h,
            width: right_w,
            height: content_height,
        };
        let right_panel_status = if right_panel_visible {
            Rect {
                x: right_panel.x,
                y: right_panel.y + right_panel.height - status_h,
                width: right_w,
                height: status_h,
            }
        } else {
            Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
        };

        let content_x = sidebar_w;
        let content_w = (viewport_w - sidebar_w - right_w).max(0.0);

        // Tab bar spans full width at the very top (no separate title bar)
        let tab_bar = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_w,
            height: tab_h,
        };

        // Breadcrumb bar between tab bar and content (only in content area)
        let breadcrumb = Rect {
            x: content_x,
            y: tab_h,
            width: content_w,
            height: breadcrumb_h,
        };

        let terminal_y = tab_h + breadcrumb_h;
        let terminal_h = (viewport_h - terminal_y - status_h).max(0.0);
        let terminal = Rect {
            x: content_x,
            y: terminal_y,
            width: content_w,
            height: terminal_h,
        };

        let pad_left = (TERMINAL_PAD_LEFT * scale).round();
        let pad_top = (TERMINAL_PAD_TOP * scale).round();
        let terminal_content = Rect {
            x: terminal.x + pad_left,
            y: terminal.y + pad_top,
            width: (terminal.width - pad_left).max(0.0),
            height: (terminal.height - pad_top).max(0.0),
        };

        Self { sidebar, tab_bar, breadcrumb, terminal, terminal_content, status_bar, right_panel, right_panel_status }
    }
}
