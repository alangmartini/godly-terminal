//! Ergonomic wrapper over raw `quad_vertices()` calls.
//!
//! `UiBuilder` collects quad and text vertices during the build phase,
//! then returns them in `finish()` for the GPU render pass.

use super::quad_renderer::{
    quad_vertices, quad_vertices_gradient, quad_vertices_gradient_h, quad_vertices_sdf,
    quad_vertices_sdf_gradient, quad_vertices_sdf_gradient_h, quad_vertices_sdf_rotated,
    QuadVertex,
};
use super::text_layout::{UiFontKind, UiTextLayout, UiTextLayoutEngine};
use super::widget::Rect;
use std::rc::Rc;

/// GitHub Dark–inspired ultra-dark palette for all UI chrome.
/// Deep blue-black tones — matches the web reference mockup.
pub mod colors {
    // Ultra-dark base colors (GitHub Dark inspired)
    pub const BG_DARK: [f32; 4] = [0.043, 0.051, 0.071, 1.0]; // #0b0d12 Chrome/sidebar
    pub const BG_BASE: [f32; 4] = [0.055, 0.063, 0.090, 1.0]; // #0e1017 Content/terminal
    pub const BG_RAISED: [f32; 4] = [0.059, 0.067, 0.090, 1.0]; // #0f1117 Elevated panels
    pub const BG_SURFACE: [f32; 4] = [0.102, 0.114, 0.145, 1.0]; // #1a1d25 Surface/hover base
    pub const BG_HOVER: [f32; 4] = [0.176, 0.200, 0.231, 1.0]; // #2d333b Hover states
    pub const BG_ACTIVE: [f32; 4] = [0.090, 0.106, 0.141, 1.0]; // #171b24 Active selection
    pub const BG_TAB_ACTIVE: [f32; 4] = [0.086, 0.098, 0.125, 1.0]; // #161920 Active tab (web)
    pub const BG_ACTIVE_ACC: [f32; 4] = [0.078, 0.090, 0.122, 1.0]; // #14171f Blue-tinted active
    pub const BG_STATUS: [f32; 4] = [0.047, 0.055, 0.078, 1.0]; // #0c0e14 Status bar
    pub const FG_BRIGHT: [f32; 4] = [0.902, 0.929, 0.953, 1.0]; // #e6edf3 Active/heading
    pub const FG_PRIMARY: [f32; 4] = [0.788, 0.820, 0.851, 1.0]; // #c9d1d9 Text
    pub const FG_SECONDARY: [f32; 4] = [0.545, 0.580, 0.620, 1.0]; // #8b949e Subtext
    pub const FG_INACTIVE: [f32; 4] = [0.569, 0.596, 0.631, 1.0]; // #9198a1 Inactive names (web)
    pub const FG_MUTED: [f32; 4] = [0.431, 0.463, 0.506, 1.0]; // #6e7681 Overlay/muted
    pub const FG_DIM: [f32; 4] = [0.333, 0.365, 0.420, 1.0]; // #555d6b Dimmed chrome
    pub const ACCENT_BLUE: [f32; 4] = [0.388, 0.400, 0.945, 1.0]; // #6366f1 Indigo
    pub const ACCENT_GREEN: [f32; 4] = [0.133, 0.773, 0.369, 1.0]; // #22c55e Green
    pub const ACCENT_PEACH: [f32; 4] = [0.961, 0.620, 0.043, 1.0]; // #f59e0b Amber
    pub const ACCENT_MAUVE: [f32; 4] = [0.545, 0.361, 0.965, 1.0]; // #8b5cf6 Violet
    pub const ACCENT_RED: [f32; 4] = [0.937, 0.267, 0.267, 1.0]; // #ef4444 Red
    pub const ACCENT_EMERALD: [f32; 4] = [0.063, 0.725, 0.506, 1.0]; // #10b981 Emerald
    pub const ACCENT_ORANGE: [f32; 4] = [0.976, 0.451, 0.086, 1.0]; // #f97316 Orange
    pub const RED_HOVER: [f32; 4] = [0.937, 0.267, 0.267, 0.8];
    pub const RED_SUBTLE: [f32; 4] = [0.15, 0.11, 0.11, 1.0];
    pub const ACCENT_GOLD: [f32; 4] = [0.902, 0.659, 0.333, 1.0]; // #e6a855 Code text
    pub const ACCENT_SKY: [f32; 4] = [0.345, 0.651, 1.0, 1.0]; // #58a6ff Links
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    // Status bar text tones — web uses much darker text than general UI
    pub const STATUS_PATH: [f32; 4] = [0.231, 0.251, 0.282, 1.0]; // #3b4048 path/muted info
    pub const STATUS_DEFAULT: [f32; 4] = [0.282, 0.310, 0.345, 1.0]; // #484f58 default status text
                                                                     // Separator / border color — ultra-dark
    pub const BORDER: [f32; 4] = [0.102, 0.114, 0.145, 1.0]; // #1a1d25 hairline border
}

/// Font size scale factors relative to the 14px base cell height.
/// Used with `text_ui_scaled`/`text_ui_bold_scaled` to match web reference pixel sizes.
pub mod font_scale {
    pub const PX9: f32 = 9.0 / 14.0; // 0.643 — tab count badges
    pub const PX10: f32 = 10.0 / 14.0; // 0.714 — shortcuts, badges, small headers
    pub const PX11: f32 = 11.0 / 14.0; // 0.786 — branch text, descriptions, status bar
    pub const PX12: f32 = 12.0 / 14.0; // 0.857 — session header, tab titles, process names
    pub const PX13: f32 = 13.0 / 14.0; // 0.929 — session names, poem stanzas
    pub const PX15: f32 = 15.0 / 14.0; // 1.071 — poem title
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
    /// When true, renders glyphs with the bold font variant.
    pub bold: bool,
    /// Which rasterizer/layout path should be used for this text run.
    pub font_kind: TextFontKind,
    /// When true, renders with the italic font face instead of a synthetic skew.
    pub italic: bool,
    /// Scale factor for glyph quads (1.0 = normal size, 0.786 = 11/14px).
    pub scale: f32,
    /// Glyph x offsets from a real text layout pass. Empty for monospace runs.
    pub glyph_offsets: Vec<f32>,
    /// Whether the renderer can use subpixel blending against `bg`, or must
    /// fall back to grayscale AA because the backdrop is mixed/unknown.
    pub composite: TextCompositeMode,
    /// Extra multiplier applied when rasterizing/drawing against a physical
    /// surface. Crop/reference scenes can set this below 1.0 to render at
    /// fixed pixel sizes instead of OS-DPI-scaled sizes.
    pub raster_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFontKind {
    TerminalMono,
    UiSans,
    UiSerif,
    UiMono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCompositeMode {
    FlatBackground,
    MixedBackground,
}

/// Thin handle passed to widget `build()` methods. Carries actual cell
/// dimensions from the font metrics so widgets can position text correctly.
pub struct UiTextRenderer {
    pub cell_width: f32,
    pub cell_height: f32,
    pub font_size_px: f32,
    pub scale: f32,
    /// Average advance width of the proportional UI font (0 if unavailable).
    pub ui_avg_advance: f32,
    /// Optional real UI text layout path. When present, widths and glyph
    /// positions come from DirectWrite instead of hand-tuned advance math.
    pub layout_engine: Option<Rc<UiTextLayoutEngine>>,
    /// Extra multiplier applied by the atlas renderer when converting the
    /// logical font metrics here into physical glyph sizes on the surface.
    pub raster_scale: f32,
}

impl UiTextRenderer {
    pub fn new(cell_width: f32, cell_height: f32, font_size_px: f32, scale: f32) -> Self {
        Self {
            cell_width,
            cell_height,
            font_size_px,
            scale,
            ui_avg_advance: 0.0,
            layout_engine: None,
            raster_scale: 1.0,
        }
    }

    /// Scale a logical pixel value to physical pixels.
    pub fn s(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Width in pixels of a string rendered in the terminal monospace font.
    pub fn text_width(&self, s: &str) -> f32 {
        s.chars().count() as f32 * self.cell_width
    }

    /// Width in pixels of a string rendered in the terminal monospace font at a given scale.
    pub fn text_width_scaled(&self, s: &str, scale: f32) -> f32 {
        self.text_width(s) * scale
    }

    /// Exact width of a string rendered in the proportional UI font.
    pub fn text_width_ui(&self, s: &str) -> f32 {
        self.layout_text(UiFontKind::Sans, s, 1.0, false, false)
            .width
    }

    /// Exact width of a string rendered in the proportional UI font at a given scale.
    pub fn text_width_ui_scaled(&self, s: &str, scale: f32) -> f32 {
        self.layout_text(UiFontKind::Sans, s, scale, false, false)
            .width
    }

    /// Exact width of a string rendered in the serif UI font at a given scale.
    pub fn text_width_serif_scaled(&self, s: &str, scale: f32, bold: bool, italic: bool) -> f32 {
        self.layout_text(UiFontKind::Serif, s, scale, bold, italic)
            .width
    }

    pub fn text_width_mono_scaled(&self, s: &str, scale: f32) -> f32 {
        self.layout_text(UiFontKind::Mono, s, scale, false, false)
            .width
    }

    pub fn mono_layout_scaled(
        &self,
        s: &str,
        scale: f32,
        bold: bool,
        italic: bool,
    ) -> UiTextLayout {
        self.layout_text(UiFontKind::Mono, s, scale, bold, italic)
    }

    pub fn glyph_offsets_for(
        &self,
        s: &str,
        scale: f32,
        bold: bool,
        italic: bool,
        font_kind: TextFontKind,
    ) -> Vec<f32> {
        match font_kind {
            TextFontKind::TerminalMono => Vec::new(),
            TextFontKind::UiSans => {
                self.layout_text(UiFontKind::Sans, s, scale, bold, italic)
                    .glyph_offsets
            }
            TextFontKind::UiSerif => {
                self.layout_text(UiFontKind::Serif, s, scale, bold, italic)
                    .glyph_offsets
            }
            TextFontKind::UiMono => {
                self.layout_text(UiFontKind::Mono, s, scale, bold, italic)
                    .glyph_offsets
            }
        }
    }

    fn layout_text(
        &self,
        font_kind: UiFontKind,
        s: &str,
        scale: f32,
        bold: bool,
        italic: bool,
    ) -> UiTextLayout {
        if let Some(engine) = &self.layout_engine {
            if let Some(layout) =
                engine.layout(font_kind, s, self.font_size_px * scale, bold, italic)
            {
                return layout;
            }
        }

        let advance = match font_kind {
            UiFontKind::Mono => self.cell_width * scale,
            UiFontKind::Sans | UiFontKind::Serif => {
                if self.ui_avg_advance > 0.0 {
                    self.ui_avg_advance * scale
                } else {
                    self.cell_width * 0.75 * scale
                }
            }
        };
        let mut glyph_offsets = Vec::with_capacity(s.chars().count());
        let mut x = 0.0;
        for _ in s.chars() {
            glyph_offsets.push(x);
            x += advance;
        }
        UiTextLayout {
            width: x,
            line_height: match font_kind {
                UiFontKind::Mono => self.cell_height * scale,
                UiFontKind::Sans | UiFontKind::Serif => self.font_size_px * scale,
            },
            glyph_offsets,
        }
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
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            color,
        ));
    }

    /// Flat rectangle with vertical gradient (no rounding).
    pub fn fill_gradient(&mut self, rect: Rect, top_color: [f32; 4], bottom_color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices_gradient(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            top_color,
            bottom_color,
        ));
    }

    /// Flat rectangle with horizontal gradient (no rounding).
    pub fn fill_gradient_h(&mut self, rect: Rect, left_color: [f32; 4], right_color: [f32; 4]) {
        self.quads.extend_from_slice(&quad_vertices_gradient_h(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            left_color,
            right_color,
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
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            color,
            radii,
            border_width,
            border_color,
            blur_radius,
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
        self.fill_sdf(
            rect,
            color,
            [radius, radius, 0.0, 0.0],
            border_width,
            border_color,
            0.0,
        );
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
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            top_color,
            bottom_color,
            [radius, radius, 0.0, 0.0],
            border_width,
            border_color,
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
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            top_color,
            bottom_color,
            [radius; 4],
            0.0,
            [0.0; 4],
        ));
    }

    /// SDF rounded rectangle with horizontal gradient fill (left → right).
    pub fn fill_rounded_gradient_h(
        &mut self,
        rect: Rect,
        left_color: [f32; 4],
        right_color: [f32; 4],
        radius: f32,
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf_gradient_h(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            left_color,
            right_color,
            [radius; 4],
            0.0,
            [0.0; 4],
        ));
    }

    /// SDF rounded rectangle with per-corner radii and horizontal gradient fill.
    pub fn fill_rounded_custom_gradient_h(
        &mut self,
        rect: Rect,
        left_color: [f32; 4],
        right_color: [f32; 4],
        radii: [f32; 4],
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf_gradient_h(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            left_color,
            right_color,
            radii,
            0.0,
            [0.0; 4],
        ));
    }

    /// SDF rounded rectangle with per-corner radii and vertical gradient fill.
    /// Useful for gradient overlays inside shapes with non-uniform rounding (e.g. tabs).
    pub fn fill_rounded_custom_gradient(
        &mut self,
        rect: Rect,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
        radii: [f32; 4],
    ) {
        self.quads.extend_from_slice(&quad_vertices_sdf_gradient(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            top_color,
            bottom_color,
            radii,
            0.0,
            [0.0; 4],
        ));
    }

    /// SDF rounded rectangle outline (border only, transparent fill).
    /// Produces smooth anti-aliased edges — use instead of stroke_rect for polish.
    pub fn stroke_rounded(&mut self, rect: Rect, radius: f32, stroke_width: f32, color: [f32; 4]) {
        self.fill_sdf(
            rect,
            [0.0, 0.0, 0.0, 0.0],
            [radius; 4],
            stroke_width,
            color,
            0.0,
        );
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
    pub fn fill_shadow(&mut self, rect: Rect, color: [f32; 4], radius: f32, blur: f32) {
        self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], blur);
    }

    /// Offset shadow for directional depth (light from top-left → shadow bottom-right).
    /// Shifts the shadow quad by `(dx, dy)` pixels while keeping the same shape.
    pub fn fill_shadow_offset(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radius: f32,
        blur: f32,
        dx: f32,
        dy: f32,
    ) {
        let offset_rect = Rect {
            x: rect.x + dx,
            y: rect.y + dy,
            width: rect.width,
            height: rect.height,
        };
        self.fill_sdf(offset_rect, color, [radius; 4], 0.0, [0.0; 4], blur);
    }

    /// Inner shadow: shadow rendered *inside* a rounded rectangle, fading inward
    /// from the edges.  Creates a recessed/carved-in depth effect — much crisper
    /// than gradient overlay approximations.
    ///
    /// Uses negative `blur_radius` to signal inner shadow mode to the SDF shader.
    pub fn fill_inner_shadow(&mut self, rect: Rect, color: [f32; 4], radius: f32, blur: f32) {
        self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], -blur);
    }

    /// Inner shadow with per-corner radii [TL, TR, BR, BL].
    pub fn fill_inner_shadow_custom(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radii: [f32; 4],
        blur: f32,
    ) {
        self.fill_sdf(rect, color, radii, 0.0, [0.0; 4], -blur);
    }

    /// Horizontal line of thickness `t`.
    pub fn hline(&mut self, x: f32, y: f32, w: f32, t: f32, color: [f32; 4]) {
        self.quads
            .extend_from_slice(&quad_vertices(x, y, w, t, self.vw, self.vh, color));
    }

    /// Vertical line of thickness `t`.
    pub fn vline(&mut self, x: f32, y: f32, h: f32, t: f32, color: [f32; 4]) {
        self.quads
            .extend_from_slice(&quad_vertices(x, y, t, h, self.vw, self.vh, color));
    }

    /// Anti-aliased horizontal line via SDF.  Produces crisp sub-pixel edges
    /// at any DPI, unlike the flat-quad `hline` which can blur at fractional scales.
    pub fn hline_aa(&mut self, x: f32, y: f32, w: f32, t: f32, color: [f32; 4]) {
        let r = (t * 0.5).min(1.0);
        self.quads.extend_from_slice(&quad_vertices_sdf(
            x, y, w, t, self.vw, self.vh, color, [r; 4], 0.0, [0.0; 4], 0.0,
        ));
    }

    /// Anti-aliased vertical line via SDF.
    pub fn vline_aa(&mut self, x: f32, y: f32, h: f32, t: f32, color: [f32; 4]) {
        let r = (t * 0.5).min(1.0);
        self.quads.extend_from_slice(&quad_vertices_sdf(
            x, y, t, h, self.vw, self.vh, color, [r; 4], 0.0, [0.0; 4], 0.0,
        ));
    }

    /// Vertical line that fades to transparent at both ends over `fade` pixels.
    /// Creates a softer separator that doesn't visually "crash" into edges.
    pub fn vline_fade(&mut self, x: f32, y: f32, h: f32, t: f32, color: [f32; 4], fade: f32) {
        let transparent = [color[0], color[1], color[2], 0.0];
        if fade > 0.0 && h > fade * 2.0 {
            // Top fade segment
            self.quads.extend_from_slice(&quad_vertices_gradient(
                x,
                y,
                t,
                fade,
                self.vw,
                self.vh,
                transparent,
                color,
            ));
            // Solid middle
            self.quads.extend_from_slice(&quad_vertices(
                x,
                y + fade,
                t,
                h - fade * 2.0,
                self.vw,
                self.vh,
                color,
            ));
            // Bottom fade segment
            self.quads.extend_from_slice(&quad_vertices_gradient(
                x,
                y + h - fade,
                t,
                fade,
                self.vw,
                self.vh,
                color,
                transparent,
            ));
        } else {
            self.vline(x, y, h, t, color);
        }
    }

    /// Horizontal line that fades to transparent at both ends over `fade` pixels.
    pub fn hline_fade(&mut self, x: f32, y: f32, w: f32, t: f32, color: [f32; 4], fade: f32) {
        let transparent = [color[0], color[1], color[2], 0.0];
        if fade > 0.0 && w > fade * 2.0 {
            // Left fade segment
            self.quads.extend_from_slice(&quad_vertices_gradient_h(
                x,
                y,
                fade,
                t,
                self.vw,
                self.vh,
                transparent,
                color,
            ));
            // Solid middle
            self.quads.extend_from_slice(&quad_vertices(
                x + fade,
                y,
                w - fade * 2.0,
                t,
                self.vw,
                self.vh,
                color,
            ));
            // Right fade segment
            self.quads.extend_from_slice(&quad_vertices_gradient_h(
                x + w - fade,
                y,
                fade,
                t,
                self.vw,
                self.vh,
                color,
                transparent,
            ));
        } else {
            self.hline(x, y, w, t, color);
        }
    }

    /// Embossed groove: dark line + light line pair that creates an inset
    /// channel effect at panel boundaries.  Much more professional than a
    /// single flat line — mimics the classic Win32/macOS panel separator look.
    /// `dark` is the shadow line, `light` is the highlight below/right of it.
    pub fn hgroove(&mut self, x: f32, y: f32, w: f32, dark: [f32; 4], light: [f32; 4]) {
        self.hline_aa(x, y, w, 1.0, dark);
        self.hline_aa(x, y + 1.0, w, 1.0, light);
    }

    /// Vertical embossed groove (dark + light pair).
    pub fn vgroove(&mut self, x: f32, y: f32, h: f32, dark: [f32; 4], light: [f32; 4]) {
        self.vline_aa(x, y, h, 1.0, dark);
        self.vline_aa(x + 1.0, y, h, 1.0, light);
    }

    /// Horizontal groove with faded ends (combines groove + fade for softer look).
    pub fn hgroove_fade(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        dark: [f32; 4],
        light: [f32; 4],
        fade: f32,
    ) {
        self.hline_fade(x, y, w, 1.0, dark, fade);
        self.hline_fade(x, y + 1.0, w, 1.0, light, fade);
    }

    /// Vertical groove with faded ends.
    pub fn vgroove_fade(
        &mut self,
        x: f32,
        y: f32,
        h: f32,
        dark: [f32; 4],
        light: [f32; 4],
        fade: f32,
    ) {
        self.vline_fade(x, y, h, 1.0, dark, fade);
        self.vline_fade(x + 1.0, y, h, 1.0, light, fade);
    }

    /// Rectangle outline (4 lines of thickness `t`, drawn inward).
    pub fn stroke_rect(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        self.hline(rect.x, rect.y, rect.width, t, color); // top
        self.hline(rect.x, rect.bottom() - t, rect.width, t, color); // bottom
        self.vline(rect.x, rect.y, rect.height, t, color); // left
        self.vline(rect.right() - t, rect.y, rect.height, t, color); // right
    }

    fn push_text_command(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        bold: bool,
        italic: bool,
        scale: f32,
        font_kind: TextFontKind,
        composite: TextCompositeMode,
    ) {
        let glyph_offsets = renderer.glyph_offsets_for(text, scale, bold, italic, font_kind);
        self.text_commands.push(TextCommand {
            text: text.to_string(),
            x,
            y,
            fg,
            bg,
            bold,
            font_kind,
            italic,
            scale,
            glyph_offsets,
            composite,
            raster_scale: renderer.raster_scale,
        });
    }

    /// Record a text draw command (monospace terminal font).
    pub fn text(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            1.0,
            TextFontKind::TerminalMono,
            TextCompositeMode::FlatBackground,
        );
    }

    pub fn text_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            1.0,
            TextFontKind::TerminalMono,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a bold text draw command (monospace terminal font).
    pub fn text_bold(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            1.0,
            TextFontKind::TerminalMono,
            TextCompositeMode::FlatBackground,
        );
    }

    pub fn text_bold_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            1.0,
            TextFontKind::TerminalMono,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a scaled monospace text draw command.
    pub fn text_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            scale,
            TextFontKind::TerminalMono,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled bold monospace text draw command.
    pub fn text_bold_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            scale,
            TextFontKind::TerminalMono,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled UI-monospace text draw command with real glyph layout.
    pub fn text_mono_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            scale,
            TextFontKind::UiMono,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled UI-monospace text draw command that should fall back to
    /// grayscale AA instead of background-specific subpixel composition.
    pub fn text_mono_scaled_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            scale,
            TextFontKind::UiMono,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a scaled bold UI-monospace text draw command with real glyph layout.
    pub fn text_mono_bold_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            scale,
            TextFontKind::UiMono,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled bold UI-monospace text draw command that should stay in
    /// grayscale AA.
    pub fn text_mono_bold_scaled_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            scale,
            TextFontKind::UiMono,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a UI text draw command (proportional sans-serif font).
    pub fn text_ui(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            1.0,
            TextFontKind::UiSans,
            TextCompositeMode::FlatBackground,
        );
    }

    pub fn text_ui_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            1.0,
            TextFontKind::UiSans,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a bold UI text draw command (proportional sans-serif font).
    pub fn text_ui_bold(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            1.0,
            TextFontKind::UiSans,
            TextCompositeMode::FlatBackground,
        );
    }

    pub fn text_ui_bold_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            1.0,
            TextFontKind::UiSans,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a scaled UI text draw command (proportional sans-serif font).
    /// `scale` controls glyph size relative to the 14px base font size.
    pub fn text_ui_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            scale,
            TextFontKind::UiSans,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled bold UI text draw command (proportional sans-serif font).
    pub fn text_ui_bold_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            scale,
            TextFontKind::UiSans,
            TextCompositeMode::FlatBackground,
        );
    }

    /// Record a scaled UI text draw command that must stay in grayscale AA
    /// because the background under the text is not a single flat color.
    pub fn text_ui_scaled_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            false,
            scale,
            TextFontKind::UiSans,
            TextCompositeMode::MixedBackground,
        );
    }

    pub fn text_ui_bold_scaled_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            true,
            false,
            scale,
            TextFontKind::UiSans,
            TextCompositeMode::MixedBackground,
        );
    }

    /// Record a scaled italic serif text draw command using a real italic font
    /// face instead of synthetic skew.
    pub fn text_serif_italic_scaled(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            true,
            scale,
            TextFontKind::UiSerif,
            TextCompositeMode::FlatBackground,
        );
    }

    pub fn text_serif_italic_scaled_mixed(
        &mut self,
        renderer: &UiTextRenderer,
        text: &str,
        x: f32,
        y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
        scale: f32,
    ) {
        self.push_text_command(
            renderer,
            text,
            x,
            y,
            fg,
            bg,
            false,
            true,
            scale,
            TextFontKind::UiSerif,
            TextCompositeMode::MixedBackground,
        );
    }

    // -- Icon helpers ---------------------------------------------------------

    /// Plus (+) icon centered in `rect`.  `t` = line thickness, `arm` = half-length.
    /// Rendered as two SDF pill shapes for smooth anti-aliased edges (matching icon_x quality).
    pub fn icon_plus(&mut self, rect: Rect, t: f32, arm: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let r = t * 0.5; // pill-shaped rounded caps
        let radii = [r; 4];
        let no_border = [0.0f32; 4];
        // Horizontal bar
        self.quads.extend_from_slice(&quad_vertices_sdf(
            cx - arm,
            cy - t / 2.0,
            arm * 2.0,
            t,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
        // Vertical bar
        self.quads.extend_from_slice(&quad_vertices_sdf(
            cx - t / 2.0,
            cy - arm,
            t,
            arm * 2.0,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
    }

    /// X (close) icon centered in `rect`, drawn as two rotated SDF pill shapes
    /// for crisp anti-aliased diagonal lines (2 quads instead of dozens).
    pub fn icon_x(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        // Each diagonal spans corner-to-corner of the size×size square
        let line_len = size * std::f32::consts::SQRT_2;
        let r = t * 0.6; // pill-shaped rounded caps
        let radii = [r; 4];
        let no_border = [0.0f32; 4];

        // Diagonal 1: top-left to bottom-right (π/4 clockwise)
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            cx,
            cy,
            line_len,
            t,
            std::f32::consts::FRAC_PI_4,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
        // Diagonal 2: top-right to bottom-left (-π/4)
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            cx,
            cy,
            line_len,
            t,
            -std::f32::consts::FRAC_PI_4,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
    }

    /// Minimize icon: a short horizontal bar centered in `rect`.
    /// Rendered as an SDF pill shape for smooth anti-aliased edges.
    pub fn icon_minimize(&mut self, rect: Rect, bar_w: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let r = t * 0.5; // pill-shaped rounded caps
        self.quads.extend_from_slice(&quad_vertices_sdf(
            cx - bar_w / 2.0,
            cy - t / 2.0,
            bar_w,
            t,
            self.vw,
            self.vh,
            color,
            [r; 4],
            0.0,
            [0.0; 4],
            0.0,
        ));
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

    /// Gear/cog icon: SDF circle ring with 6 rectangular teeth around the
    /// perimeter and a small center dot.  Each tooth is a rotated SDF pill.
    pub fn icon_gear(&mut self, rect: Rect, outer: f32, _inner: f32, fg: [f32; 4], _bg: [f32; 4]) {
        let (cx, cy) = rect.center();
        let radius = outer * 0.38; // ring mid-radius
        let ring_t = outer * 0.12; // ring stroke thickness
        let mid_size = radius * 2.0;
        let mid_rect = Rect {
            x: cx - mid_size / 2.0,
            y: cy - mid_size / 2.0,
            width: mid_size,
            height: mid_size,
        };
        self.stroke_rounded(mid_rect, mid_size / 2.0, ring_t, fg);
        // Center dot
        let dot_r = outer * 0.10;
        let dot_rect = Rect {
            x: cx - dot_r,
            y: cy - dot_r,
            width: dot_r * 2.0,
            height: dot_r * 2.0,
        };
        self.fill_rounded(dot_rect, fg, dot_r);
        // 6 teeth — short rounded rects pointing outward from the ring
        let tooth_len = outer * 0.18;
        let tooth_w = outer * 0.14;
        let tooth_r = tooth_w * 0.3;
        let tooth_center_r = radius + tooth_len * 0.35;
        let no_border = [0.0f32; 4];
        for i in 0..6u32 {
            let angle = i as f32 * std::f32::consts::TAU / 6.0;
            let tx = cx + tooth_center_r * angle.cos();
            let ty = cy + tooth_center_r * angle.sin();
            self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
                tx,
                ty,
                tooth_len,
                tooth_w,
                angle,
                self.vw,
                self.vh,
                fg,
                [tooth_r; 4],
                0.0,
                no_border,
                0.0,
            ));
        }
    }

    /// Folder icon: rectangle body with a small tab on the top-left.
    /// Drawn as two SDF rounded rects for crisp anti-aliased edges.
    pub fn icon_folder(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let hw = rect.width * 0.38;
        let hh = rect.height * 0.28;
        let r = hw * 0.15;
        // Main body
        let body = Rect {
            x: cx - hw,
            y: cy - hh * 0.6,
            width: hw * 2.0,
            height: hh * 2.0 * 0.8,
        };
        self.stroke_rounded(body, r, t, color);
        // Tab on top-left
        let tab_w = hw * 0.7;
        let tab_h = hh * 0.5;
        let tab = Rect {
            x: cx - hw,
            y: cy - hh * 0.6 - tab_h + t * 0.5,
            width: tab_w,
            height: tab_h + t * 0.5,
        };
        self.fill_rounded_custom(tab, color, [r, r * 0.6, 0.0, 0.0]);
    }

    /// Git branch icon: a forked line representing version control.
    /// Two circles (nodes) connected by lines — bottom node forks upward
    /// into two diagonal arms ending at top nodes.
    pub fn icon_git_branch(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let hw = rect.width * 0.30;
        let hh = rect.height * 0.36;
        let node_r = t * 1.2;
        let r = t * 0.5;
        let radii = [r; 4];
        let no_border = [0.0f32; 4];
        // Bottom node (trunk)
        let bottom = (cx, cy + hh);
        let node_rect = |x: f32, y: f32| Rect {
            x: x - node_r,
            y: y - node_r,
            width: node_r * 2.0,
            height: node_r * 2.0,
        };
        self.fill_rounded(node_rect(bottom.0, bottom.1), color, node_r);
        // Top-left node (branch)
        let top_left = (cx - hw, cy - hh);
        self.fill_rounded(node_rect(top_left.0, top_left.1), color, node_r);
        // Top-right node (trunk tip)
        let top_right = (cx + hw * 0.3, cy - hh);
        self.fill_rounded(node_rect(top_right.0, top_right.1), color, node_r);
        // Trunk line: bottom → top-right
        let dx = top_right.0 - bottom.0;
        let dy = top_right.1 - bottom.1;
        let len = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);
        let mcx = (bottom.0 + top_right.0) / 2.0;
        let mcy = (bottom.1 + top_right.1) / 2.0;
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            mcx, mcy, len, t, angle, self.vw, self.vh, color, radii, 0.0, no_border, 0.0,
        ));
        // Branch line: mid-trunk → top-left
        let fork_y = cy + hh * 0.1;
        let fork_x = cx + hw * 0.3 * 0.1; // slight offset matching trunk angle
        let dx2 = top_left.0 - fork_x;
        let dy2 = top_left.1 - fork_y;
        let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        let angle2 = dy2.atan2(dx2);
        let mcx2 = (fork_x + top_left.0) / 2.0;
        let mcy2 = (fork_y + top_left.1) / 2.0;
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            mcx2, mcy2, len2, t, angle2, self.vw, self.vh, color, radii, 0.0, no_border, 0.0,
        ));
    }

    /// Disclosure triangle (▾): filled downward-pointing triangle centered in `rect`.
    /// Used for collapsible section headers in the sidebar (Zed/VS Code pattern).
    /// `size` is the bounding triangle dimension; rendered as 3 SDF pill arms.
    pub fn icon_disclosure_down(&mut self, rect: Rect, size: f32, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let half = size * 0.45;
        // Three corners of the triangle: top-left, top-right, bottom-center
        let tl = (cx - half, cy - half * 0.35);
        let tr = (cx + half, cy - half * 0.35);
        let bc = (cx, cy + half * 0.55);
        let r = t * 0.5;
        let radii = [r; 4];
        let no_border = [0.0f32; 4];
        // Arm: top-left → bottom-center
        let dx1 = bc.0 - tl.0;
        let dy1 = bc.1 - tl.1;
        let len1 = (dx1 * dx1 + dy1 * dy1).sqrt();
        let a1 = dy1.atan2(dx1);
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            (tl.0 + bc.0) / 2.0,
            (tl.1 + bc.1) / 2.0,
            len1,
            t,
            a1,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
        // Arm: top-right → bottom-center
        let dx2 = bc.0 - tr.0;
        let dy2 = bc.1 - tr.1;
        let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        let a2 = dy2.atan2(dx2);
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            (tr.0 + bc.0) / 2.0,
            (tr.1 + bc.1) / 2.0,
            len2,
            t,
            a2,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
        // Arm: top-left → top-right (closes the top edge)
        let dx3 = tr.0 - tl.0;
        let dy3 = tr.1 - tl.1;
        let len3 = (dx3 * dx3 + dy3 * dy3).sqrt();
        let a3 = dy3.atan2(dx3);
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            (tl.0 + tr.0) / 2.0,
            (tl.1 + tr.1) / 2.0,
            len3,
            t,
            a3,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
    }

    /// Draw a right-pointing chevron (›) using two SDF rotated pills.
    /// Used for breadcrumb path separators.
    pub fn icon_chevron_right(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let half = rect.height * 0.30;
        let r = t * 0.5;
        let radii = [r; 4];
        let no_border = [0.0f32; 4];
        // Top arm: center-right ← top-left
        let top = (cx - half * 0.4, cy - half);
        let tip = (cx + half * 0.4, cy);
        let dx = tip.0 - top.0;
        let dy = tip.1 - top.1;
        let len = (dx * dx + dy * dy).sqrt();
        let a = dy.atan2(dx);
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            (top.0 + tip.0) / 2.0,
            (top.1 + tip.1) / 2.0,
            len,
            t,
            a,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
        // Bottom arm: center-right → bottom-left
        let bot = (cx - half * 0.4, cy + half);
        let dx2 = tip.0 - bot.0;
        let dy2 = tip.1 - bot.1;
        let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        let a2 = dy2.atan2(dx2);
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            (bot.0 + tip.0) / 2.0,
            (bot.1 + tip.1) / 2.0,
            len2,
            t,
            a2,
            self.vw,
            self.vh,
            color,
            radii,
            0.0,
            no_border,
            0.0,
        ));
    }

    /// Draw a terminal icon: monitor outline + prompt chevron + cursor inside.
    /// Uses SDF rotated pills for the chevron so it scales cleanly at any size.
    pub fn icon_terminal(&mut self, rect: Rect, t: f32, color: [f32; 4]) {
        let (cx, cy) = rect.center();
        let hw = rect.width * 0.36;
        let hh = rect.height * 0.28;
        // Monitor outline
        let monitor = Rect {
            x: cx - hw,
            y: cy - hh,
            width: hw * 2.0,
            height: hh * 2.0,
        };
        self.stroke_rounded(monitor, hw * 0.12, t, color);
        // Prompt chevron ">" — two SDF rotated pills forming a clean V-shape.
        // Each arm runs from the midpoint to a corner of the chevron.
        let caret_x = cx - hw * 0.4;
        let caret_mid_x = cx - hw * 0.05;
        let caret_top_y = cy - hh * 0.45;
        let caret_bot_y = cy + hh * 0.45;
        let caret_mid_y = cy;
        // Upper arm: (caret_x, caret_top_y) → (caret_mid_x, caret_mid_y)
        let dx_top = caret_mid_x - caret_x;
        let dy_top = caret_mid_y - caret_top_y;
        let len_top = (dx_top * dx_top + dy_top * dy_top).sqrt();
        let angle_top = dy_top.atan2(dx_top);
        let mcx_top = (caret_x + caret_mid_x) / 2.0;
        let mcy_top = (caret_top_y + caret_mid_y) / 2.0;
        let r = t * 0.5;
        let radii = [r; 4];
        let no_border = [0.0f32; 4];
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            mcx_top, mcy_top, len_top, t, angle_top, self.vw, self.vh, color, radii, 0.0,
            no_border, 0.0,
        ));
        // Lower arm: (caret_mid_x, caret_mid_y) → (caret_x, caret_bot_y)
        let dx_bot = caret_x - caret_mid_x;
        let dy_bot = caret_bot_y - caret_mid_y;
        let len_bot = (dx_bot * dx_bot + dy_bot * dy_bot).sqrt();
        let angle_bot = dy_bot.atan2(dx_bot);
        let mcx_bot = (caret_mid_x + caret_x) / 2.0;
        let mcy_bot = (caret_mid_y + caret_bot_y) / 2.0;
        self.quads.extend_from_slice(&quad_vertices_sdf_rotated(
            mcx_bot, mcy_bot, len_bot, t, angle_bot, self.vw, self.vh, color, radii, 0.0,
            no_border, 0.0,
        ));
        // Cursor line (horizontal bar next to the caret)
        let cursor_x = cx + hw * 0.05;
        let cursor_y = cy + hh * 0.35;
        self.hline_aa(cursor_x, cursor_y, hw * 0.5, t, color);
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
        ui.fill(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            colors::BG_BASE,
        );
        let (quads, _) = ui.finish();
        assert_eq!(quads.len(), 6);
    }

    #[test]
    fn builder_stroke_rect_produces_24_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.stroke_rect(
            Rect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 40.0,
            },
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
        let tr = UiTextRenderer::new(8.0, 16.0, 14.0, 1.0);
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
            Rect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
            2.0,
            6.0,
            colors::WHITE,
        );
        let (quads, _) = ui.finish();
        // hline (6) + vline (6) = 12
        assert_eq!(quads.len(), 12);
    }

    #[test]
    fn icon_minimize_produces_vertices() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_minimize(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 46.0,
                height: 32.0,
            },
            10.0,
            1.0,
            colors::FG_PRIMARY,
        );
        let (quads, _) = ui.finish();
        assert_eq!(quads.len(), 6);
    }

    #[test]
    fn icon_maximize_produces_sdf_quad() {
        let mut ui = UiBuilder::new(100.0, 100.0);
        ui.icon_maximize(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 46.0,
                height: 32.0,
            },
            10.0,
            1.0,
            colors::FG_PRIMARY,
        );
        let (quads, _) = ui.finish();
        // Single SDF rounded rect outline = 6 vertices
        assert_eq!(quads.len(), 6);
    }
}
