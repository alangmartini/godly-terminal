//! Recursive split pane rendering from LayoutNode tree.

use std::collections::HashMap;
use godly_layout_core::{LayoutNode, SplitDirection};
use godly_protocol::types::RichGridData;
use crate::terminal_renderer::TerminalRenderer;
use crate::ui::widget::Rect;

/// Render a layout tree into the given render pass.
///
/// Each leaf node renders its terminal grid using the corresponding renderer.
/// Split nodes subdivide the rect and recurse.
pub fn render_layout(
    node: &LayoutNode,
    rect: Rect,
    renderers: &mut HashMap<String, TerminalRenderer>,
    grids: &HashMap<String, RichGridData>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    render_pass: &mut wgpu::RenderPass<'_>,
) {
    match node {
        LayoutNode::Leaf { terminal_id } => {
            if let (Some(grid), Some(renderer)) = (grids.get(terminal_id), renderers.get_mut(terminal_id)) {
                // TODO: need full viewport dimensions for proper split rendering
                renderer.render(
                    device,
                    queue,
                    render_pass,
                    grid,
                    rect.width as u32,
                    rect.height as u32,
                    rect.x,
                    rect.y,
                );
            }
        }
        LayoutNode::Split { direction, ratio, first, second } => {
            let (r1, r2) = split_rect(rect, *direction, *ratio);
            render_layout(first, r1, renderers, grids, device, queue, render_pass);
            render_layout(second, r2, renderers, grids, device, queue, render_pass);
        }
        LayoutNode::ContentPane { .. } => {
            // File viewer pane — not yet implemented
        }
    }
}

fn split_rect(rect: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
    const DIVIDER: f32 = 2.0;
    match direction {
        SplitDirection::Horizontal => {
            let w1 = (rect.width * ratio - DIVIDER / 2.0).max(0.0);
            let w2 = (rect.width * (1.0 - ratio) - DIVIDER / 2.0).max(0.0);
            (
                Rect { x: rect.x, y: rect.y, width: w1, height: rect.height },
                Rect { x: rect.x + w1 + DIVIDER, y: rect.y, width: w2, height: rect.height },
            )
        }
        SplitDirection::Vertical => {
            let h1 = (rect.height * ratio - DIVIDER / 2.0).max(0.0);
            let h2 = (rect.height * (1.0 - ratio) - DIVIDER / 2.0).max(0.0);
            (
                Rect { x: rect.x, y: rect.y, width: rect.width, height: h1 },
                Rect { x: rect.x, y: rect.y + h1 + DIVIDER, width: rect.width, height: h2 },
            )
        }
    }
}
