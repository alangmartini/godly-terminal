//! Status bar: process name, cwd, terminal dimensions.

use super::quad_renderer::{quad_vertices, QuadVertex};
use super::widget::Rect;

const BG_COLOR: [f32; 4] = [0.09, 0.09, 0.11, 1.0];

pub struct StatusBar {
    pub process_name: String,
    pub terminal_size: (u16, u16),
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            process_name: String::new(),
            terminal_size: (24, 80),
        }
    }

    pub fn build_quads(&self, bar: Rect, vw: f32, vh: f32) -> Vec<QuadVertex> {
        let mut verts = Vec::new();
        verts.extend_from_slice(&quad_vertices(bar.x, bar.y, bar.width, bar.height, vw, vh, BG_COLOR));
        verts
    }
}
