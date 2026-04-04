//! Layout constants and retained taffy-based computation for the shell chrome.

use super::widget::Rect;
use taffy::prelude::{AvailableSpace, Dimension, Display, FlexDirection, Size, Style};
use taffy::{NodeId, TaffyTree};

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

#[derive(Debug, Clone, Copy)]
struct ShellNodes {
    root: NodeId,
    tab_bar: NodeId,
    body: NodeId,
    sidebar: NodeId,
    center: NodeId,
    breadcrumb: NodeId,
    terminal: NodeId,
    right_panel: NodeId,
    right_panel_content: NodeId,
    right_panel_status: NodeId,
    status_bar: NodeId,
}

/// Retained flexbox tree for shell chrome.
///
/// The old shell layout path was a pile of manual rectangle math. Keeping the
/// top-level regions in a persistent taffy tree gives us a real layout layer we
/// can extend incrementally instead of continuing to hand-solve geometry.
pub struct ShellLayoutEngine {
    tree: TaffyTree<()>,
    nodes: ShellNodes,
}

impl ShellLayoutEngine {
    pub fn new() -> Self {
        let mut tree = TaffyTree::new();

        let tab_bar = tree.new_leaf(Style::default()).expect("tab_bar node");
        let sidebar = tree.new_leaf(Style::default()).expect("sidebar node");
        let breadcrumb = tree.new_leaf(Style::default()).expect("breadcrumb node");
        let terminal = tree.new_leaf(Style::default()).expect("terminal node");
        let status_bar = tree.new_leaf(Style::default()).expect("status_bar node");

        let center = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                },
                &[breadcrumb, terminal, status_bar],
            )
            .expect("center node");

        let right_panel_content = tree
            .new_leaf(Style::default())
            .expect("right_panel_content node");
        let right_panel_status = tree
            .new_leaf(Style::default())
            .expect("right_panel_status node");
        let right_panel = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                &[right_panel_content, right_panel_status],
            )
            .expect("right_panel node");

        let body = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                },
                &[sidebar, center, right_panel],
            )
            .expect("body node");

        let root = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &[tab_bar, body],
            )
            .expect("root node");

        Self {
            tree,
            nodes: ShellNodes {
                root,
                tab_bar,
                body,
                sidebar,
                center,
                breadcrumb,
                terminal,
                right_panel,
                right_panel_content,
                right_panel_status,
                status_bar,
            },
        }
    }

    pub fn compute(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        sidebar_visible: bool,
        right_panel_visible: bool,
        status_bar_visible: bool,
        sidebar_width: f32,
        right_panel_width: f32,
        scale: f32,
    ) -> ShellLayout {
        let viewport_w = viewport_w.max(0.0);
        let viewport_h = viewport_h.max(0.0);
        let tab_h = (TAB_BAR_HEIGHT * scale).round();
        let status_h = if status_bar_visible {
            (STATUS_BAR_HEIGHT * scale).round()
        } else {
            0.0
        };
        let breadcrumb_h = (BREADCRUMB_HEIGHT * scale).round();
        let sidebar_w = if sidebar_visible {
            (sidebar_width * scale).round()
        } else {
            0.0
        };
        let right_w = if right_panel_visible {
            (right_panel_width * scale).round()
        } else {
            0.0
        };

        self.set_style(
            self.nodes.root,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: px(viewport_w),
                    height: px(viewport_h),
                },
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.tab_bar,
            Style {
                display: Display::Flex,
                size: Size {
                    width: px(viewport_w),
                    height: px(tab_h),
                },
                flex_shrink: 0.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.body,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                size: Size {
                    width: px(viewport_w),
                    height: Dimension::Auto,
                },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.sidebar,
            if sidebar_visible {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: px(sidebar_w),
                        height: Dimension::Auto,
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.center,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.breadcrumb,
            Style {
                display: Display::Flex,
                size: Size {
                    width: Dimension::Percent(1.0),
                    height: px(breadcrumb_h),
                },
                flex_shrink: 0.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.terminal,
            Style {
                display: Display::Flex,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.right_panel,
            if right_panel_visible {
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    size: Size {
                        width: px(right_w),
                        height: Dimension::Auto,
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.right_panel_content,
            Style {
                display: Display::Flex,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.right_panel_status,
            if right_panel_visible {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: Dimension::Percent(1.0),
                        height: px(status_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.status_bar,
            if status_bar_visible {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: Dimension::Percent(1.0),
                        height: px(status_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );

        self.tree
            .compute_layout(
                self.nodes.root,
                Size {
                    width: AvailableSpace::Definite(viewport_w),
                    height: AvailableSpace::Definite(viewport_h),
                },
            )
            .expect("shell layout should compute");

        let tab_bar = self.absolute_rect_for(&[self.nodes.tab_bar]);
        let status_bar = self.absolute_rect_for(&[
            self.nodes.body,
            self.nodes.center,
            self.nodes.status_bar,
        ]);
        let sidebar = if sidebar_visible {
            self.absolute_rect_for(&[self.nodes.body, self.nodes.sidebar])
        } else {
            zero_rect()
        };
        let breadcrumb =
            self.absolute_rect_for(&[self.nodes.body, self.nodes.center, self.nodes.breadcrumb]);
        let terminal =
            self.absolute_rect_for(&[self.nodes.body, self.nodes.center, self.nodes.terminal]);
        let right_panel = if right_panel_visible {
            self.absolute_rect_for(&[self.nodes.body, self.nodes.right_panel])
        } else {
            zero_rect()
        };
        let right_panel_status = if right_panel_visible {
            self.absolute_rect_for(&[
                self.nodes.body,
                self.nodes.right_panel,
                self.nodes.right_panel_status,
            ])
        } else {
            zero_rect()
        };

        let pad_left = (TERMINAL_PAD_LEFT * scale).round();
        let pad_top = (TERMINAL_PAD_TOP * scale).round();
        let terminal_content = Rect {
            x: terminal.x + pad_left,
            y: terminal.y + pad_top,
            width: (terminal.width - pad_left).max(0.0),
            height: (terminal.height - pad_top).max(0.0),
        };

        ShellLayout {
            sidebar,
            tab_bar,
            breadcrumb,
            terminal,
            terminal_content,
            status_bar,
            right_panel,
            right_panel_status,
        }
    }

    fn set_style(&mut self, node: NodeId, style: Style) {
        self.tree
            .set_style(node, style)
            .expect("layout node style should update");
    }

    fn absolute_rect_for(&self, path: &[NodeId]) -> Rect {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut width = 0.0;
        let mut height = 0.0;

        for node in path {
            let layout = self.tree.layout(*node).expect("layout node should exist");
            x += layout.location.x;
            y += layout.location.y;
            width = layout.size.width;
            height = layout.size.height;
        }

        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

impl Default for ShellLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellLayout {
    /// Compatibility wrapper for older call sites and tests.
    pub fn compute(
        viewport_w: f32,
        viewport_h: f32,
        sidebar_visible: bool,
        right_panel_visible: bool,
        status_bar_visible: bool,
        sidebar_width: f32,
        right_panel_width: f32,
        scale: f32,
    ) -> Self {
        ShellLayoutEngine::new().compute(
            viewport_w,
            viewport_h,
            sidebar_visible,
            right_panel_visible,
            status_bar_visible,
            sidebar_width,
            right_panel_width,
            scale,
        )
    }
}

fn px(value: f32) -> Dimension {
    Dimension::Length(value.max(0.0))
}

fn hidden_style() -> Style {
    Style {
        display: Display::None,
        ..Default::default()
    }
}

fn zero_rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() < 0.01,
            "expected {a} ~= {b}, diff={}",
            (a - b).abs()
        );
    }

    #[test]
    fn taffy_layout_matches_expected_visible_regions() {
        let layout = ShellLayout::compute(1200.0, 800.0, true, true, true, 200.0, 380.0, 1.0);

        approx_eq(layout.tab_bar.height, 36.0);
        approx_eq(layout.status_bar.y, 774.0);
        approx_eq(layout.sidebar.width, 200.0);
        approx_eq(layout.sidebar.y, 36.0);
        approx_eq(layout.right_panel.x, 820.0);
        approx_eq(layout.right_panel.width, 380.0);
        approx_eq(layout.terminal.x, 200.0);
        approx_eq(layout.terminal.y, 36.0);
        approx_eq(layout.terminal.width, 620.0);
        approx_eq(layout.terminal.height, 738.0);
        approx_eq(layout.terminal_content.x, 208.0);
        approx_eq(layout.terminal_content.y, 42.0);
    }

    #[test]
    fn hidden_panels_collapse_cleanly() {
        let layout = ShellLayout::compute(1200.0, 800.0, false, false, true, 200.0, 380.0, 1.0);

        approx_eq(layout.sidebar.width, 0.0);
        approx_eq(layout.right_panel.width, 0.0);
        approx_eq(layout.terminal.x, 0.0);
        approx_eq(layout.terminal.width, 1200.0);
    }

    #[test]
    fn scale_factor_updates_fixed_regions() {
        let layout = ShellLayout::compute(1200.0, 800.0, true, true, true, 200.0, 380.0, 1.5);

        approx_eq(layout.tab_bar.height, 54.0);
        approx_eq(layout.status_bar.height, 39.0);
        approx_eq(layout.sidebar.width, 300.0);
        approx_eq(layout.right_panel.width, 570.0);
        approx_eq(layout.terminal_content.x, 312.0);
        approx_eq(layout.terminal_content.y, 63.0);
    }

    #[test]
    fn hidden_status_bar_returns_height_to_terminal() {
        let layout = ShellLayout::compute(1200.0, 800.0, true, false, false, 200.0, 380.0, 1.0);

        approx_eq(layout.status_bar.height, 0.0);
        approx_eq(layout.terminal.height, 764.0);
    }
}
