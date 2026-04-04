use super::tab_bar::{
    BUTTON_WIDTH, TAB_GAP, TAB_INSET_V, TAB_MARGIN_LEFT, TAB_MAX_WIDTH, TAB_MIN_WIDTH,
};
use super::widget::Rect;
use taffy::prelude::{AvailableSpace, Dimension, Display, FlexDirection, Size, Style};
use taffy::{NodeId, TaffyTree};

const CONTROLS_GAP: f32 = 12.0;
const NEW_TAB_SLOT_WIDTH: f32 = 32.0;
const NEW_TAB_BUTTON_SIZE: f32 = 24.0;
const INDICATOR_RESERVE: f32 = 150.0;

#[derive(Debug, Clone, Copy)]
pub struct TabBarLayoutConfig {
    pub show_brand: bool,
    pub show_indicators: bool,
    pub show_controls: bool,
    pub show_new_tab: bool,
    pub content_sized_tabs: bool,
    pub tabs_padding_left: f32,
    pub tab_inset_v: f32,
}

impl Default for TabBarLayoutConfig {
    fn default() -> Self {
        Self {
            show_brand: true,
            show_indicators: true,
            show_controls: true,
            show_new_tab: true,
            content_sized_tabs: false,
            tabs_padding_left: TAB_MARGIN_LEFT,
            tab_inset_v: TAB_INSET_V,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TabBarNodes {
    root: NodeId,
    brand: NodeId,
    tabs_strip: NodeId,
    indicators: NodeId,
    controls_gap: NodeId,
    controls: NodeId,
    minimize: NodeId,
    maximize: NodeId,
    close: NodeId,
    new_tab_slot: NodeId,
}

#[derive(Debug, Clone)]
pub struct TabBarLayout {
    pub brand: Rect,
    pub tabs: Vec<Rect>,
    pub indicators: Rect,
    pub controls_gap: Rect,
    pub new_tab: Rect,
    pub buttons: [Rect; 3],
}

pub struct TabBarLayoutEngine {
    tree: TaffyTree<()>,
    nodes: TabBarNodes,
    tab_slots: Vec<NodeId>,
}

impl TabBarLayoutEngine {
    pub fn new() -> Self {
        let mut tree = TaffyTree::new();

        let brand = tree.new_leaf(Style::default()).expect("brand node");
        let tabs_strip = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..Default::default()
                },
                &[],
            )
            .expect("tabs strip node");
        let indicators = tree.new_leaf(Style::default()).expect("indicators node");
        let controls_gap = tree.new_leaf(Style::default()).expect("controls gap node");

        let minimize = tree.new_leaf(Style::default()).expect("minimize node");
        let maximize = tree.new_leaf(Style::default()).expect("maximize node");
        let close = tree.new_leaf(Style::default()).expect("close node");
        let controls = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                &[minimize, maximize, close],
            )
            .expect("controls node");

        let new_tab_slot = tree.new_leaf(Style::default()).expect("new tab slot node");
        tree.set_children(tabs_strip, &[new_tab_slot])
            .expect("new tab slot child");

        let root = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                },
                &[brand, tabs_strip, indicators, controls_gap, controls],
            )
            .expect("tab bar root node");

        Self {
            tree,
            nodes: TabBarNodes {
                root,
                brand,
                tabs_strip,
                indicators,
                controls_gap,
                controls,
                minimize,
                maximize,
                close,
                new_tab_slot,
            },
            tab_slots: Vec::new(),
        }
    }

    pub fn compute(
        &mut self,
        bar: Rect,
        sidebar_width: f32,
        tab_count: usize,
        tab_widths: &[f32],
        scale: f32,
        config: TabBarLayoutConfig,
    ) -> TabBarLayout {
        self.ensure_tab_slots(tab_count);

        let bar_h = bar.height.max(0.0);
        let brand_w = if config.show_brand {
            sidebar_width.max(0.0)
        } else {
            0.0
        };
        let indicator_w = if config.show_indicators {
            (INDICATOR_RESERVE * scale).round()
        } else {
            0.0
        };
        let controls_gap_w = if config.show_controls {
            (CONTROLS_GAP * scale).round()
        } else {
            0.0
        };
        let controls_w = if config.show_controls {
            (BUTTON_WIDTH * scale).round() * 3.0
        } else {
            0.0
        };
        let new_tab_slot_w = if config.show_new_tab {
            (NEW_TAB_SLOT_WIDTH * scale).round()
        } else {
            0.0
        };
        let tab_margin_left = (config.tabs_padding_left * scale).round();
        let gap = (TAB_GAP * scale).round();

        self.set_style(
            self.nodes.root,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                size: Size {
                    width: px(bar.width),
                    height: px(bar.height),
                },
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.brand,
            if brand_w > 0.0 {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: px(brand_w),
                        height: px(bar_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.tabs_strip,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: taffy::Rect {
                    left: lp(tab_margin_left),
                    right: lp(0.0),
                    top: lp(0.0),
                    bottom: lp(0.0),
                },
                ..Default::default()
            },
        );
        self.set_style(
            self.nodes.indicators,
            if config.show_indicators {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: px(indicator_w),
                        height: px(bar_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.controls_gap,
            if config.show_controls {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: px(controls_gap_w),
                        height: px(bar_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        self.set_style(
            self.nodes.controls,
            if config.show_controls {
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    size: Size {
                        width: px(controls_w),
                        height: px(bar_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );

        let active_slots = self.tab_slots[..tab_count].to_vec();
        for (index, slot) in active_slots.iter().copied().enumerate() {
            self.set_style(
                slot,
                if config.content_sized_tabs {
                    let tab_w = tab_widths
                        .get(index)
                        .copied()
                        .unwrap_or((TAB_MIN_WIDTH * scale).round());
                    let slot_w = if index + 1 < tab_count {
                        tab_w + gap
                    } else {
                        tab_w
                    };
                    Style {
                        display: Display::Flex,
                        size: Size {
                            width: px(slot_w),
                            height: px(bar_h),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    }
                } else {
                    Style {
                        display: Display::Flex,
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        flex_basis: px(0.0),
                        min_size: Size {
                            width: px((TAB_MIN_WIDTH * scale).round()),
                            height: px(bar_h),
                        },
                        max_size: Size {
                            width: px((TAB_MAX_WIDTH * scale).round()),
                            height: px(bar_h),
                        },
                        ..Default::default()
                    }
                },
            );
        }
        self.set_style(
            self.nodes.new_tab_slot,
            if config.show_new_tab {
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: px(new_tab_slot_w),
                        height: px(bar_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                hidden_style()
            },
        );
        for button in [self.nodes.minimize, self.nodes.maximize, self.nodes.close] {
            self.set_style(
                button,
                if config.show_controls {
                    Style {
                        display: Display::Flex,
                        size: Size {
                            width: px((BUTTON_WIDTH * scale).round()),
                            height: px(bar_h),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    }
                } else {
                    hidden_style()
                },
            );
        }

        self.tree
            .compute_layout(
                self.nodes.root,
                Size {
                    width: AvailableSpace::Definite(bar.width),
                    height: AvailableSpace::Definite(bar.height),
                },
            )
            .expect("tab bar layout should compute");

        let brand = if brand_w > 0.0 {
            self.absolute_rect_for(&[self.nodes.brand], bar)
        } else {
            zero_rect()
        };
        let indicators = if config.show_indicators {
            self.absolute_rect_for(&[self.nodes.indicators], bar)
        } else {
            zero_rect()
        };
        let controls_gap = if config.show_controls {
            self.absolute_rect_for(&[self.nodes.controls_gap], bar)
        } else {
            zero_rect()
        };
        let buttons = if config.show_controls {
            [
                self.absolute_rect_for(&[self.nodes.controls, self.nodes.minimize], bar),
                self.absolute_rect_for(&[self.nodes.controls, self.nodes.maximize], bar),
                self.absolute_rect_for(&[self.nodes.controls, self.nodes.close], bar),
            ]
        } else {
            [zero_rect(), zero_rect(), zero_rect()]
        };

        let inset = (config.tab_inset_v * scale).round();
        let tabs = self.tab_slots[..tab_count]
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let slot = self.absolute_rect_for(&[self.nodes.tabs_strip, *node], bar);
                let width = if index + 1 < tab_count {
                    (slot.width - gap).max(0.0)
                } else {
                    slot.width
                };
                Rect {
                    x: slot.x,
                    y: slot.y + inset,
                    width,
                    height: (slot.height - inset).max(0.0),
                }
            })
            .collect();

        let new_tab = if config.show_new_tab {
            let new_slot =
                self.absolute_rect_for(&[self.nodes.tabs_strip, self.nodes.new_tab_slot], bar);
            let new_btn_sz = (NEW_TAB_BUTTON_SIZE * scale).round();
            Rect {
                x: new_slot.right() - new_btn_sz,
                y: new_slot.y + (new_slot.height - new_btn_sz) / 2.0,
                width: new_btn_sz,
                height: new_btn_sz,
            }
        } else {
            zero_rect()
        };

        TabBarLayout {
            brand,
            tabs,
            indicators,
            controls_gap,
            new_tab,
            buttons,
        }
    }

    fn ensure_tab_slots(&mut self, tab_count: usize) {
        while self.tab_slots.len() < tab_count {
            let node = self.tree.new_leaf(Style::default()).expect("tab slot node");
            self.tab_slots.push(node);
        }

        let mut children = self.tab_slots[..tab_count].to_vec();
        children.push(self.nodes.new_tab_slot);
        self.tree
            .set_children(self.nodes.tabs_strip, &children)
            .expect("tabs strip children");
    }

    fn set_style(&mut self, node: NodeId, style: Style) {
        self.tree
            .set_style(node, style)
            .expect("tab bar layout style should update");
    }

    fn absolute_rect_for(&self, path: &[NodeId], bar: Rect) -> Rect {
        let mut x = bar.x;
        let mut y = bar.y;
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

impl Default for TabBarLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn px(value: f32) -> Dimension {
    Dimension::Length(value.max(0.0))
}

fn lp(value: f32) -> taffy::LengthPercentage {
    taffy::LengthPercentage::Length(value.max(0.0))
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
    fn retained_tab_layout_matches_expected_sections() {
        let mut layout_engine = TabBarLayoutEngine::new();
        let layout = layout_engine.compute(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1100.0,
                height: 36.0,
            },
            200.0,
            5,
            &[],
            1.0,
            TabBarLayoutConfig::default(),
        );

        approx_eq(layout.brand.width, 200.0);
        approx_eq(layout.indicators.width, 150.0);
        approx_eq(layout.controls_gap.width, 12.0);
        approx_eq(layout.buttons[0].width, 46.0);
        approx_eq(layout.buttons[2].right(), 1100.0);
        assert_eq!(layout.tabs.len(), 5);
        assert!(layout.tabs[0].x >= 202.0);
        assert!(layout.tabs[4].right() <= layout.new_tab.x);
        approx_eq(layout.new_tab.width, 24.0);
        approx_eq(layout.new_tab.height, 24.0);
    }

    #[test]
    fn hidden_brand_reclaims_space_for_tabs() {
        let mut layout_engine = TabBarLayoutEngine::new();
        let with_brand = layout_engine.compute(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1100.0,
                height: 36.0,
            },
            200.0,
            5,
            &[],
            1.0,
            TabBarLayoutConfig::default(),
        );
        let without_brand = layout_engine.compute(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1100.0,
                height: 36.0,
            },
            200.0,
            5,
            &[],
            1.0,
            TabBarLayoutConfig {
                show_brand: false,
                ..Default::default()
            },
        );

        approx_eq(without_brand.brand.width, 0.0);
        assert!(without_brand.tabs[0].x < with_brand.tabs[0].x);
        assert!(without_brand.tabs[0].width >= with_brand.tabs[0].width);
    }

    #[test]
    fn hidden_optional_sections_zero_their_rects() {
        let mut layout_engine = TabBarLayoutEngine::new();
        let layout = layout_engine.compute(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1100.0,
                height: 36.0,
            },
            200.0,
            5,
            &[],
            1.0,
            TabBarLayoutConfig {
                show_brand: false,
                show_indicators: false,
                show_controls: false,
                show_new_tab: false,
                content_sized_tabs: false,
                tabs_padding_left: TAB_MARGIN_LEFT,
                tab_inset_v: TAB_INSET_V,
            },
        );

        approx_eq(layout.brand.width, 0.0);
        approx_eq(layout.indicators.width, 0.0);
        approx_eq(layout.controls_gap.width, 0.0);
        approx_eq(layout.new_tab.width, 0.0);
        approx_eq(layout.buttons[0].width, 0.0);
        assert!(layout.tabs[0].x <= 12.0);
    }

    #[test]
    fn content_sized_tabs_use_intrinsic_widths() {
        let mut layout_engine = TabBarLayoutEngine::new();
        let tab_widths = [92.0, 132.0, 88.0];
        let layout = layout_engine.compute(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 900.0,
                height: 36.0,
            },
            0.0,
            3,
            &tab_widths,
            1.0,
            TabBarLayoutConfig {
                show_brand: false,
                show_indicators: false,
                show_controls: false,
                show_new_tab: false,
                content_sized_tabs: true,
                tabs_padding_left: 2.0,
                tab_inset_v: 0.0,
            },
        );

        approx_eq(layout.tabs[0].x, 2.0);
        approx_eq(layout.tabs[0].width, 92.0);
        approx_eq(layout.tabs[1].x, 100.0);
        approx_eq(layout.tabs[1].width, 132.0);
        approx_eq(layout.tabs[2].x, 238.0);
        approx_eq(layout.tabs[2].width, 88.0);
        approx_eq(layout.tabs[0].height, 36.0);
    }
}
