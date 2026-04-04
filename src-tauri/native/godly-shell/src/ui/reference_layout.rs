use super::builder::UiTextRenderer;
use super::widget::Rect;
use taffy::prelude::{AvailableSpace, Dimension, Display, FlexDirection, Size, Style};
use taffy::{NodeId, TaffyTree};

pub const BLOCK_COUNT: usize = 16;
pub const BLOCK_INTRO: usize = 0;
pub const BLOCK_VERIFICATION_HEADING: usize = 1;
pub const BLOCK_VERIFICATION_BULLET: usize = 2;
pub const BLOCK_SMOKE_BULLET: usize = 3;
pub const BLOCK_RESIDUAL_HEADING: usize = 4;
pub const BLOCK_RESIDUAL_PARAGRAPH: usize = 5;
pub const BLOCK_RESIDUAL_NUMBERED: usize = 6;
pub const BLOCK_USER_FUN: usize = 7;
pub const BLOCK_USER_COMPACT: usize = 8;
pub const BLOCK_THOUGHTS_ONE: usize = 9;
pub const BLOCK_PARAGRAPH_TONE: usize = 10;
pub const BLOCK_COMMAND: usize = 11;
pub const BLOCK_THOUGHTS_TWO: usize = 12;
pub const BLOCK_PARAGRAPH_COLLAPSE: usize = 13;
pub const BLOCK_EDITING: usize = 14;
pub const BLOCK_CURSOR: usize = 15;

const CONTENT_PAD_X: f32 = 20.0;
const CONTENT_PAD_TOP: f32 = 12.0;
const CONTENT_PAD_BOTTOM: f32 = 20.0;
const BODY_LINE_HEIGHT: f32 = 13.0 * 1.55; // web: fontSize 13, lineHeight 1.55
const PARAGRAPH_LINE_HEIGHT: f32 = 13.0 * 1.6; // web: fontSize 13, lineHeight 1.6
const SUB_ROW_HEIGHT: f32 = 12.0 * 1.5 + 2.0; // web: fontSize 12, inherited lineHeight 1.5, padding "1px 0"
const HEADING_HEIGHT: f32 = 14.0 * 1.5; // web: fontSize 14, inherited lineHeight 1.5
const USER_MESSAGE_HEIGHT: f32 = 13.0 * 1.5 + 12.0; // web: fontSize 13, inherited lineHeight 1.5, padding "6px 12px"
const THOUGHTS_HEIGHT: f32 = 12.0 * 1.5; // web: fontSize 12, inherited lineHeight 1.5
const COMMAND_HEIGHT: f32 = 12.0 * 1.5 + 12.0; // web: fontSize 12, inherited lineHeight 1.5, padding "6px 10px"
const EDITING_HEIGHT: f32 = 12.0 * 1.5; // web: fontSize 12, inherited lineHeight 1.5
const CURSOR_HEIGHT: f32 = 16.0;

#[derive(Debug, Clone, Copy)]
pub struct ReferencePaneLayout {
    pub content: Rect,
    pub blocks: [Rect; BLOCK_COUNT],
}

pub struct ReferencePaneLayoutEngine {
    tree: TaffyTree<()>,
    root: NodeId,
    blocks: [NodeId; BLOCK_COUNT],
}

impl ReferencePaneLayoutEngine {
    pub fn new() -> Self {
        let mut tree = TaffyTree::new();
        let blocks = std::array::from_fn(|_| {
            tree.new_leaf(Style::default())
                .expect("reference block node")
        });
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &blocks,
            )
            .expect("reference root");
        Self { tree, root, blocks }
    }

    /// Compute layout with optional per-block line counts for text wrapping.
    /// `line_counts` maps block index → number of text lines. Blocks not in
    /// the map default to 1 line.
    pub fn compute_wrapped(
        &mut self,
        pane: Rect,
        text: &UiTextRenderer,
        line_counts: &[usize; BLOCK_COUNT],
    ) -> ReferencePaneLayout {
        self.compute_inner(pane, text, Some(line_counts))
    }

    pub fn compute(&mut self, pane: Rect, text: &UiTextRenderer) -> ReferencePaneLayout {
        self.compute_inner(pane, text, None)
    }

    fn compute_inner(
        &mut self,
        pane: Rect,
        text: &UiTextRenderer,
        line_counts: Option<&[usize; BLOCK_COUNT]>,
    ) -> ReferencePaneLayout {
        let s = |v: f32| text.s(v);
        let content = Rect {
            x: pane.x + s(CONTENT_PAD_X),
            y: pane.y + s(CONTENT_PAD_TOP),
            width: (pane.width - s(CONTENT_PAD_X * 2.0)).max(0.0),
            height: (pane.height - s(CONTENT_PAD_TOP + CONTENT_PAD_BOTTOM)).max(0.0),
        };

        self.set_style(
            self.root,
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: px(content.width),
                    height: px(content.height),
                },
                ..Default::default()
            },
        );

        let lc = |idx: usize| -> usize {
            line_counts.map_or(1, |lc| lc[idx].max(1))
        };

        // Block heights and raw CSS margins (margin_top, margin_bottom) before collapsing.
        // In CSS block flow (the web content area is NOT display:flex), adjacent
        // vertical margins collapse: gap = max(prev_bottom, curr_top), not sum.
        let blocks_raw: [(f32, f32, f32); BLOCK_COUNT] = [
            (s(BODY_LINE_HEIGHT) * lc(BLOCK_INTRO) as f32, 4.0, 4.0),
            (s(HEADING_HEIGHT), 18.0, 8.0),
            (s(BODY_LINE_HEIGHT) * lc(BLOCK_VERIFICATION_BULLET) as f32
                + s(2.0 + SUB_ROW_HEIGHT * 4.0), 4.0, 4.0),
            (s(BODY_LINE_HEIGHT) * lc(BLOCK_SMOKE_BULLET) as f32, 4.0, 4.0),
            (s(HEADING_HEIGHT), 18.0, 8.0),
            (s(PARAGRAPH_LINE_HEIGHT) * lc(BLOCK_RESIDUAL_PARAGRAPH) as f32, 8.0, 8.0),
            (s(BODY_LINE_HEIGHT) * lc(BLOCK_RESIDUAL_NUMBERED) as f32, 4.0, 4.0),
            (s(USER_MESSAGE_HEIGHT), 6.0, 6.0),
            (s(USER_MESSAGE_HEIGHT), 6.0, 6.0),
            (s(THOUGHTS_HEIGHT), 8.0, 6.0), // web: margin "8px 0 6px"
            (s(PARAGRAPH_LINE_HEIGHT) * lc(BLOCK_PARAGRAPH_TONE) as f32, 8.0, 8.0),
            (s(COMMAND_HEIGHT), 8.0, 8.0),
            (s(THOUGHTS_HEIGHT), 8.0, 6.0), // web: margin "8px 0 6px"
            (s(PARAGRAPH_LINE_HEIGHT) * lc(BLOCK_PARAGRAPH_COLLAPSE) as f32, 8.0, 8.0),
            (s(EDITING_HEIGHT), 6.0, 6.0),
            (s(CURSOR_HEIGHT), 8.0, 0.0),
        ];

        // Simulate CSS margin collapsing: gap between adjacent blocks = max(prev_bottom, curr_top).
        // We achieve this by setting margin_bottom = 0 for all blocks, and
        // collapsed_top = max(prev_bottom_css, curr_top_css) for blocks 1..N.
        // Block 0 keeps its original top margin (no collapsing with the container
        // because the container has overflow:auto which creates a BFC).
        for i in 0..BLOCK_COUNT {
            let (height, css_top, _css_bottom) = blocks_raw[i];
            let collapsed_top = if i == 0 {
                css_top
            } else {
                let prev_bottom = blocks_raw[i - 1].2;
                f32::max(prev_bottom, css_top)
            };
            self.set_style(self.blocks[i], block_style(height, collapsed_top, 0.0, text));
        }

        self.tree
            .compute_layout(
                self.root,
                Size {
                    width: AvailableSpace::Definite(content.width),
                    height: AvailableSpace::Definite(content.height),
                },
            )
            .expect("reference pane layout should compute");

        let blocks =
            std::array::from_fn(|index| self.absolute_rect_for(self.blocks[index], content));

        ReferencePaneLayout { content, blocks }
    }

    fn set_style(&mut self, node: NodeId, style: Style) {
        self.tree
            .set_style(node, style)
            .expect("reference pane layout style should update");
    }

    fn absolute_rect_for(&self, node: NodeId, content: Rect) -> Rect {
        let layout = self.tree.layout(node).expect("reference block layout");
        Rect {
            x: content.x + layout.location.x,
            y: content.y + layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        }
    }
}

impl Default for ReferencePaneLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn block_style(height: f32, margin_top: f32, margin_bottom: f32, text: &UiTextRenderer) -> Style {
    Style {
        display: Display::Flex,
        size: Size {
            width: Dimension::Percent(1.0),
            height: px(height),
        },
        flex_shrink: 0.0,
        margin: taffy::Rect {
            left: taffy::LengthPercentageAuto::Length(0.0),
            right: taffy::LengthPercentageAuto::Length(0.0),
            top: taffy::LengthPercentageAuto::Length(text.s(margin_top)),
            bottom: taffy::LengthPercentageAuto::Length(text.s(margin_bottom)),
        },
        ..Default::default()
    }
}

fn px(value: f32) -> Dimension {
    Dimension::Length(value.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_layout_uses_web_content_inset_and_ordered_blocks() {
        let mut engine = ReferencePaneLayoutEngine::new();
        let text = UiTextRenderer::new(8.0, 16.0, 14.0, 1.0);
        let pane = Rect {
            x: 200.0,
            y: 36.0,
            width: 1200.0,
            height: 900.0,
        };
        let layout = engine.compute(pane, &text);

        assert_eq!(layout.content.x, 220.0);
        assert_eq!(layout.content.y, 48.0);
        assert_eq!(layout.content.width, 1160.0);
        assert!(layout.blocks[BLOCK_VERIFICATION_HEADING].y > layout.blocks[BLOCK_INTRO].y);
        assert!(layout.blocks[BLOCK_COMMAND].y > layout.blocks[BLOCK_PARAGRAPH_TONE].y);
        assert!(layout.blocks[BLOCK_CURSOR].y > layout.blocks[BLOCK_EDITING].y);
    }
}
