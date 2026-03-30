//! Ergonomic wrapper over raw `quad_vertices()` calls.
//!
//! `UiBuilder` collects quad and text vertices during the build phase,
//! then returns them in `finish()` for the GPU render pass.

use super::quad_renderer::{quad_vertices, quad_vertices_gradient, quad_vertices_gradient_h, quad_vertices_sdf, quad_vertices_sdf_gradient, QuadVertex};
use super::widget::Rect;

/// Catppuccin Mocha palette for all UI chrome.
pub mod colors {
    // Catppuccin Mocha base colors
    pub const BG_DARK: [f32; 4] = [0.071, 0.071, 0.106, 1.0];      // #11111b Crust
    pub const BG_BASE: [f32; 4] = [0.118, 0.118, 0.180, 1.0];      // #1e1e2e Base
    pub const BG_RAISED: [f32; 4] = [0.114, 0.114, 0.153, 1.0];    // #1c1c27 (between crust and mantle)
    pub const BG_SURFACE: [f32; 4] = [0.180, 0.184, 0.239, 1.0];   // #2e2f3d Surface0
    pub const BG_HOVER: [f32; 4] = [0.224, 0.227, 0.290, 1.0];     // #393b4a Surface1
    pub const BG_ACTIVE: [f32; 4] = [0.149, 0.153, 0.208, 1.0];    // #262735 Mantle-ish
    pub const BG_ACTIVE_ACC: [f32; 4] = [0.180, 0.204, 0.290, 1.0]; // blue-tinted surface
    pub const FG_PRIMARY: [f32; 4] = [0.804, 0.839, 0.957, 1.0];   // #cdd6f4 Text
    pub const FG_SECONDARY: [f32; 4] = [0.702, 0.729, 0.835, 1.0]; // #b3bad5 Subtext1
    pub const FG_MUTED: [f32; 4] = [0.427, 0.443, 0.537, 1.0];     // #6d7189 Overlay0
    pub const ACCENT_BLUE: [f32; 4] = [0.537, 0.706, 0.980, 1.0];  // #89b4fa Blue
    pub const ACCENT_GREEN: [f32; 4] = [0.651, 0.890, 0.631, 1.0]; // #a6e3a1 Green
    pub const ACCENT_PEACH: [f32; 4] = [0.980, 0.702, 0.529, 1.0]; // #fab387 Peach
    pub const ACCENT_MAUVE: [f32; 4] = [0.796, 0.651, 0.969, 1.0]; // #cba6f7 Mauve
    pub const ACCENT_RED: [f32; 4] = [0.953, 0.545, 0.659, 1.0];   // #f38ba8 Red
    pub const RED_HOVER: [f32; 4] = [0.953, 0.545, 0.659, 0.8];
    pub const RED_SUBTLE: [f32; 4] = [0.15, 0.12, 0.12, 1.0];
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    // Separator / border color
    pub const BORDER: [f32; 4] = [0.220, 0.224, 0.280, 0.8];       // Surface0-ish, more visible
}

/// A deferred text draw command. Characters are rasterized through the
/// glyph atlas at render time so UI chrome gets the same ClearType quality
/// as terminal content.
#[derive(Debug, Clone)]
pub struct TextCommand {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
}

/// Thin handle passed to widget `build()` methods. Carries actual cell
/// dimensions from the font metrics so widgets can position text correctly.
pub struct UiTextRenderer {
    pub cell_width: f32,
    pub cell_height: f32,
    pub scale: f32,
}

impl UiTextRenderer {
    pub fn new(cell_width: f32, cell_height: f32, scale: f32) -> Self {
        Self { cell_width, cell_height, scale }
    }

    /// Scale a logical pixel value to physical pixels.
    pub fn s(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Width in pixels of a string rendered in the terminal font.
    pub fn text_width(&self, s: &str) -> f32 {
        s.chars().count() as f32 * self.cell_width
    }
}

/// Collects quad vertices and text commands for UI chrome, then returns them
/// together via `finish()`.
pub struct UiBuilder {
    quads: Vec<QuadVertex>,
    text_commands: Vec<TextCommand>,
    vw: f32,
    vh: f32,
}

impl UiBuilder {
    pub fn new(vw: f32, vh: f32) -> Self {
        Self {
            quads: Vec::new(),
            text_commands: Vec::new(),
            vw,
            vh,
        }
    }

    /// Viewport dimensions in physical pixels.
    pub fn viewport(&self) -> (f32, f32) {
        (self.vw, self.vh)
    }

    /// Solid filled rectangle (flat, no rounding).
    pub fn fill(&mut self, rect: Rect, color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, color,
        ));
    }

    /// Flat rectangle with vertical gradient (no rounding).
    pub fn fill_gradient(&mut self, rect: Rect, top_color: [f32; 4], bottom_color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices_gradient(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, top_color, bottom_color,
        ));
    }

    /// Flat rectangle with horizontal gradient (no rounding).
    pub fn fill_gradient_h(&mut self, rect: Rect, left_color: [f32; 4], right_color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices_gradient_h(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, left_color, right_color,
        ));
    }

    // -- SDF rounded rectangle methods ----------------------------------------

    /// Core SDF method — all other rounded/shadow methods delegate here.
    fn fill_sdf(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radii: [f32; 4],
        border_width: f32,
        border_color: [f32; 4],
        blur_radius: f32,
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, color,
            radii, border_width, border_color, blur_radius,
        ));
    }

    /// Filled rounded rectangle with uniform corner radius.
    pub fn fill_rounded(&mut self, rect: Rect, color: [f32; 4], radius: f32) {
        self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], 0.0);
    }

    /// Filled rounded rectangle with border (uniform radius).
    pub fn fill_rounded_bordered(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radius: f32,
        border_width: f32,
        border_color: [f32; 4],
    ) {
        self.fill_sdf(rect, color, [radius; 4], border_width, border_color, 0.0);
    }

    /// Rounded rectangle with only top corners rounded (for tabs).
    pub fn fill_rounded_top(&mut self, rect: Rect, color: [f32; 4], radius: f32) {
        self.fill_sdf(rect, color, [radius, radius, 0.0, 0.0], 0.0, [0.0; 4], 0.0);
    }

    /// Rounded rectangle with only top corners + border (for active tabs).
    pub fn fill_rounded_top_bordered(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radius: f32,
        border_width: f32,
        border_color: [f32; 4],
    ) {
        self.fill_sdf(rect, color, [radius, radius, 0.0, 0.0], border_width, border_color, 0.0);
    }

    /// Rounded rectangle with per-corner radii [TL, TR, BR, BL].
    pub fn fill_rounded_custom(&mut self, rect: Rect, color: [f32; 4], radii: [f32; 4]) {
        self.fill_sdf(rect, color, radii, 0.0, [0.0; 4], 0.0);
    }

    /// SDF rounded rectangle with top-only rounding, gradient fill, and border.
    pub fn fill_rounded_top_gradient(
        &mut self,
        rect: Rect,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
        radius: f32,
        border_width: f32,
        border_color: [f32; 4],
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf_gradient(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, top_color, bottom_color,
            [radius, radius, 0.0, 0.0], border_width, border_color,
        ));
    }

    /// SDF rounded rectangle with vertical gradient fill.
    pub fn fill_rounded_gradient(
        &mut self,
        rect: Rect,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
        radius: f32,
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf_gradient(
            rect.x, rect.y, rect.width, rect.height,
            self.vw, self.vh, top_color, bottom_color,
            [radius; 4], 0.0, [0.0; 4],
        ));
    }

    /// SDF rounded rectangle outline (border only, transparent fill).
    /// Produces smooth anti-aliased edges — use instead of stroke_rect for polish.
    pub fn stroke_rounded(
        &mut self,
        rect: Rect,
        radius: f32,
        stroke_width: f32,
        color: [f32; 4],
    ) {
        self.fill_sdf(rect, [0.0, 0.0, 0.0, 0.0], [radius; 4], stroke_width, color, 0.0);
    }

    /// SDF rounded rectangle outline with per-corner radii.
    pub fn stroke_rounded_custom(
        &mut self,
        rect: Rect,
        radii: [f32; 4],
        stroke_width: f32,
        color: [f32; 4],
    ) {
        self.fill_sdf(rect, [0.0, 0.0, 0.0, 0.0], radii, stroke_width, color, 0.0);
    }

    /// Soft shadow/glow behind an element (SDF with wide blur).
    pub fn fill_shadow(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radius: f32,
        blur: f32,
    ) {
        self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], blur);
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

    /// Record a text draw command. The actual glyph rasterization happens
    /// later via the atlas pipeline.
    pub fn text(
        &mut self,
        _renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.text_commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            fg,
            bg,
        });
    }

    // -- Icon helpers ---------------------------------------------------------

    /// Plus (+) icon centered in `rect`.  `t` = line thickness, `arm` = half-length.
    pub fn icon_plus(&mut self, rect: Rect, t: f32, arm: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        self.hline(cx - arm, cy - t / 2.0, arm * 2.0, t, color);
        self.vline(cx - t / 2.0, cy - arm, arm * 2.0, t, color);
    }

    /// X (close) icon centered in `rect`, drawn as overlapping SDF circles
    /// along two diagonals for smooth anti-aliased rendering.
    pub fn icon_x(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let half = size / 2.0;
        let dot_r = t * 0.65;
        let d = dot_r * 2.0;
        // Use enough steps for smooth coverage (overlap circles along diagonal)
        let steps = (size / (t * 0.6)).ceil().max(4.0) as usize;
        for i in 0..=steps {
            let frac = i as f32 / steps as f32;
            let dx = -half + frac * size;
            let dy = -half + frac * size;
            // Diagonal 1: top-left to bottom-right
            self.fill_rounded(
                Rect { x: cx + dx - dot_r, y: cy + dy - dot_r, width: d, height: d },
                color, dot_r,
            );
            // Diagonal 2: top-right to bottom-left
            self.fill_rounded(
                Rect { x: cx - dx - dot_r, y: cy + dy - dot_r, width: d, height: d },
                color, dot_r,
            );
        }
    }

    /// Minimize icon: a short horizontal bar centered in `rect`.
    pub fn icon_minimize(&mut self, rect: Rect, bar_w: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        self.hline(cx - bar_w / 2.0, cy - t / 2.0, bar_w, t, color);
    }

    /// Maximize icon: a small rounded square outline centered in `rect` (SDF anti-aliased).
    pub fn icon_maximize(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let inner = Rect {
            x: cx - size / 2.0,
            y: cy - size / 2.0,
            width: size,
            height: size,
        };
        self.stroke_rounded(inner, t * 0.8, t, color);
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

    /// Consume the builder and return quad vertices + text commands.
    pub fn finish(self) -> (Vec<QuadVertex>, Vec<TextCommand>) {
        (self.quads, self.text_commands)
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
    fn builder_text_produces_commands() {
        let mut ui = UiBuilder::new(800.0, 600.0);
        let tr = UiTextRenderer::new(8.0, 16.0, 1.0);
        ui.text(&tr, "Hi", 10.0, 10.0, colors::FG_PRIMARY, colors::BG_BASE);
        let (_, text_cmds) = ui.finish();
        assert_eq!(text_cmds.len(), 1);
        assert_eq!(text_cmds[0].text, "Hi");
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
    fn icon_maximize_produces_sdf_quad() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_maximize(
            Rect { x: 0.0, y: 0.0, width: 46.0, height: 32.0 },
            10.0, 1.0, colors::FG_PRIMARY,
        );
        let (quads, _) = ui.finish();
        // Single SDF rounded rect outline = 6 vertices
        assert_eq!(quads.len(), 6);
    }
}
