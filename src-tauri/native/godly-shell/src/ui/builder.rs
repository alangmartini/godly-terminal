//! Ergonomic wrapper over raw `quad_vertices()` calls.
//!
//! `UiBuilder` collects quad and text vertices during the build phase,
//! then returns them in `finish()` for the GPU render pass.

use godly_terminal_surface::atlas_vertex_builder::CellVertex;

use super::quad_renderer::{quad_vertices, QuadVertex};
use super::widget::Rect;

/// Palette of dark-theme colors used across all UI chrome.
pub mod colors {
    pub const BG_DARK: [f32; 4] = [0.08, 0.08, 0.10, 1.0];
    pub const BG_BASE: [f32; 4] = [0.09, 0.09, 0.11, 1.0];
    pub const BG_RAISED: [f32; 4] = [0.11, 0.11, 0.13, 1.0];
    pub const BG_SURFACE: [f32; 4] = [0.14, 0.14, 0.17, 1.0];
    pub const BG_HOVER: [f32; 4] = [0.22, 0.22, 0.26, 1.0];
    pub const BG_ACTIVE: [f32; 4] = [0.16, 0.16, 0.19, 1.0];
    pub const BG_ACTIVE_ACC: [f32; 4] = [0.18, 0.22, 0.30, 1.0];
    pub const FG_PRIMARY: [f32; 4] = [0.85, 0.85, 0.88, 1.0];
    pub const FG_SECONDARY: [f32; 4] = [0.75, 0.75, 0.78, 1.0];
    pub const FG_MUTED: [f32; 4] = [0.50, 0.50, 0.55, 1.0];
    pub const ACCENT_BLUE: [f32; 4] = [0.40, 0.60, 1.0, 1.0];
    pub const ACCENT_RED: [f32; 4] = [0.85, 0.25, 0.25, 1.0];
    pub const RED_HOVER: [f32; 4] = [0.95, 0.30, 0.30, 1.0];
    pub const RED_SUBTLE: [f32; 4] = [0.15, 0.12, 0.12, 1.0];
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
}

/// Placeholder for a future UI text renderer that rasterises glyphs
/// through the same atlas pipeline used for terminal cells.
///
/// Currently a no-op: text vertices are collected but rendering
/// requires atlas pipeline integration (tracked separately).
pub struct UiTextRenderer {
    cell_width: f32,
    cell_height: f32,
}

impl UiTextRenderer {
    pub fn new(cell_width: f32, cell_height: f32) -> Self {
        Self { cell_width, cell_height }
    }

    /// Produce cell vertices for a string at pixel position `(x, y)`.
    ///
    /// Each character occupies one cell-width horizontally.  The atlas
    /// UV is set to zero (no glyph texture yet), so only the background
    /// colour is visible until the atlas pipeline is wired up.
    pub fn layout_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        vw: f32,
        vh: f32,
    ) -> Vec<CellVertex> {
        let mut verts = Vec::with_capacity(text.len() * 6);
        let mut cx = x;
        for _ch in text.chars() {
            let x0 = cx / vw * 2.0 - 1.0;
            let y0 = -(y / vh * 2.0 - 1.0);
            let x1 = (cx + self.cell_width) / vw * 2.0 - 1.0;
            let y1 = -((y + self.cell_height) / vh * 2.0 - 1.0);

            // 6 vertices (2 triangles), UV = 0 (no glyph yet)
            let tl = CellVertex { position: [x0, y0], uv: [0.0, 0.0], fg_color: fg, bg_color: bg };
            let tr = CellVertex { position: [x1, y0], uv: [0.0, 0.0], fg_color: fg, bg_color: bg };
            let bl = CellVertex { position: [x0, y1], uv: [0.0, 0.0], fg_color: fg, bg_color: bg };
            let br = CellVertex { position: [x1, y1], uv: [0.0, 0.0], fg_color: fg, bg_color: bg };
            verts.extend_from_slice(&[tl, tr, bl, bl, tr, br]);

            cx += self.cell_width;
        }
        verts
    }
}

/// Collects quad and text vertices for UI chrome, then returns them
/// together via `finish()`.
pub struct UiBuilder {
    quads: Vec<QuadVertex>,
    text: Vec<CellVertex>,
    vw: f32,
    vh: f32,
}

impl UiBuilder {
    pub fn new(vw: f32, vh: f32) -> Self {
        Self {
            quads: Vec::new(),
            text: Vec::new(),
            vw,
            vh,
        }
    }

    /// Viewport dimensions in physical pixels.
    pub fn viewport(&self) -> (f32, f32) {
        (self.vw, self.vh)
    }

    /// Solid filled rectangle.
    pub fn fill(&mut self, rect: Rect, color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, color,
        ));
    }

    /// Horizontal line of thickness `t`.
    pub fn hline(&mut self, x: f32, y: f32, w: f32, t: f32, color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices(
            x, y, w, t, self.vw, self.vh, color,
        ));
    }

    /// Vertical line of thickness `t`.
    pub fn vline(&mut self, x: f32, y: f32, h: f32, t: f32, color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices(
            x, y, t, h, self.vw, self.vh, color,
        ));
    }

    /// Rectangle outline (4 lines of thickness `t`, drawn inward).
    pub fn stroke_rect(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        self.hline(rect.x, rect.y, rect.width, t, color); // top
        self.hline(rect.x, rect.bottom() - t, rect.width, t, color); // bottom
        self.vline(rect.x, rect.y, rect.height, t, color); // left
        self.vline(rect.right() - t, rect.y, rect.height, t, color); // right
    }

    /// Render a text string at `(x, y)` using the UI text renderer.
    pub fn text(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        let verts = renderer.layout_text(text, x, y, fg, bg, self.vw, self.vh);
        self.text.extend_from_slice(&verts);
    }

    // -- Icon helpers ---------------------------------------------------------

    /// Plus (+) icon centered in `rect`.  `t` = line thickness, `arm` = half-length.
    pub fn icon_plus(&mut self, rect: Rect, t: f32, arm: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        self.hline(cx - arm, cy - t / 2.0, arm * 2.0, t, color);
        self.vline(cx - t / 2.0, cy - arm, arm * 2.0, t, color);
    }

    /// X (close) icon centered in `rect`, drawn as two pixel-stepped diagonals.
    pub fn icon_x(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let half = size / 2.0;
        let steps = (half / t).ceil() as usize;
        let step = half / steps as f32;
        for i in 0..steps {
            let off = i as f32 * step;
            // top-left to bottom-right diagonal
            self.fill(Rect { x: cx - half + off, y: cy - half + off, width: t, height: t }, color);
            // top-right to bottom-left diagonal
            self.fill(Rect { x: cx + half - off - t, y: cy - half + off, width: t, height: t }, color);
        }
    }

    /// Minimize icon: a short horizontal bar centered in `rect`.
    pub fn icon_minimize(&mut self, rect: Rect, bar_w: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        self.hline(cx - bar_w / 2.0, cy - t / 2.0, bar_w, t, color);
    }

    /// Maximize icon: a small square outline centered in `rect`.
    pub fn icon_maximize(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let inner = Rect {
            x: cx - size / 2.0,
            y: cy - size / 2.0,
            width: size,
            height: size,
        };
        self.stroke_rect(inner, t, color);
    }

    /// Gear icon (approximated): filled outer circle minus filled inner circle.
    /// Drawn as a filled square (outer) on top of a bg-colored square (inner).
    pub fn icon_gear(&mut self, rect: Rect, outer: f32, inner: f32, fg: [f32; 4], bg: [f32; 4]) {
        let (cx, cy) = rect.center();
        self.fill(
            Rect { x: cx - outer / 2.0, y: cy - outer / 2.0, width: outer, height: outer },
            fg,
        );
        self.fill(
            Rect { x: cx - inner / 2.0, y: cy - inner / 2.0, width: inner, height: inner },
            bg,
        );
    }

    /// Consume the builder and return collected vertices.
    pub fn finish(self) -> (Vec<QuadVertex>, Vec<CellVertex>) {
        (self.quads, self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_fill_produces_six_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.fill(Rect { x: 0.0, y: 0.0, width: 50.0, height: 50.0 }, colors::BG_BASE);
        let (quads, _) = ui.finish();
        assert_eq!(quads.len(), 6);
    }

    #[test]
    fn builder_stroke_rect_produces_24_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.stroke_rect(
            Rect { x: 10.0, y: 10.0, width: 40.0, height: 40.0 },
            1.0,
            colors::WHITE,
        );
        let (quads, _) = ui.finish();
        // 4 lines * 6 vertices each = 24
        assert_eq!(quads.len(), 24);
    }

    #[test]
    fn builder_text_produces_cell_vertices() {
        let mut ui = UiBuilder::new(800.0, 600.0);
        let tr = UiTextRenderer::new(8.0, 16.0);
        ui.text(&tr, "Hi", 10.0, 10.0, colors::FG_PRIMARY, colors::BG_BASE);
        let (_, text) = ui.finish();
        // "Hi" = 2 characters * 6 vertices = 12
        assert_eq!(text.len(), 12);
    }

    #[test]
    fn builder_viewport_returns_dimensions() {
        let ui = UiBuilder::new(1920.0, 1080.0);
        assert_eq!(ui.viewport(), (1920.0, 1080.0));
    }

    #[test]
    fn builder_finish_empty() {
        let ui = UiBuilder::new(100.0, 100.0);
        let (quads, text) = ui.finish();
        assert!(quads.is_empty());
        assert!(text.is_empty());
    }

    #[test]
    fn icon_plus_produces_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_plus(
            Rect { x: 10.0, y: 10.0, width: 20.0, height: 20.0 },
            2.0, 6.0, colors::WHITE,
        );
        let (quads, _) = ui.finish();
        // hline (6) + vline (6) = 12
        assert_eq!(quads.len(), 12);
    }

    #[test]
    fn icon_minimize_produces_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_minimize(
            Rect { x: 0.0, y: 0.0, width: 46.0, height: 32.0 },
            10.0, 1.0, colors::FG_PRIMARY,
        );
        let (quads, _) = ui.finish();
        assert_eq!(quads.len(), 6);
    }

    #[test]
    fn icon_maximize_produces_stroke() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_maximize(
            Rect { x: 0.0, y: 0.0, width: 46.0, height: 32.0 },
            10.0, 1.0, colors::FG_PRIMARY,
        );
        let (quads, _) = ui.finish();
        // stroke_rect = 24 vertices
        assert_eq!(quads.len(), 24);
    }
}
