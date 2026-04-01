use godly_terminal_surface::{
    atlas_shader::{AtlasPipeline, AtlasShaderProgram},
    atlas_vertex_builder::{self, CellVertex},
    glyph_atlas::GlyphAtlas,
    glyph_cache::GlyphKey,
    glyph_rasterizer::GlyphRasterizer,
    font_metrics::FontMetrics,
    Color,
};
use godly_protocol::types::RichGridData;

use crate::ui::builder::TextCommand;

pub struct TerminalRenderer {
    pipeline: AtlasPipeline,
    glyph_atlas: GlyphAtlas,
    rasterizer: Box<dyn GlyphRasterizer>,
    /// Optional proportional sans-serif rasterizer for UI chrome labels.
    ui_rasterizer: Option<Box<dyn GlyphRasterizer>>,
    font_metrics: FontMetrics,
    /// Average advance width of the UI font (for layout estimation).
    ui_avg_advance: f32,
    default_fg: Color,
    default_bg: Color,
}

impl TerminalRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_metrics: FontMetrics,
        rasterizer: Box<dyn GlyphRasterizer>,
    ) -> Self {
        let pipeline = AtlasPipeline::new(device, queue, format);
        let phys = font_metrics.scaled_for_render();
        let glyph_atlas = GlyphAtlas::new(phys.cell_width, phys.cell_height, phys.baseline_offset);

        Self {
            pipeline,
            glyph_atlas,
            rasterizer,
            ui_rasterizer: None,
            font_metrics,
            ui_avg_advance: 0.0,
            default_fg: Color::new(0.788, 0.820, 0.851, 1.0),  // GitHub Dark Text #c9d1d9
            default_bg: Color::new(0.055, 0.063, 0.090, 1.0), // GitHub Dark Base #0e1017
        }
    }

    /// Set the proportional UI font rasterizer (e.g., Segoe UI).
    pub fn set_ui_rasterizer(&mut self, mut rasterizer: Box<dyn GlyphRasterizer>) {
        let phys = self.font_metrics.scaled_for_render();
        let metrics = rasterizer.measure(phys.font_size);
        self.ui_avg_advance = metrics.average_advance;
        self.ui_rasterizer = Some(rasterizer);
    }

    /// Average advance width of the UI proportional font.
    pub fn ui_avg_advance(&self) -> f32 {
        self.ui_avg_advance
    }

    /// Build per-character advance table for printable ASCII (0x20..=0x7E)
    /// using the proportional UI font. Returns empty vec if no UI rasterizer.
    pub fn build_ui_char_advances(&mut self) -> Vec<f32> {
        let phys = self.font_metrics.scaled_for_render();
        if let Some(ref mut rast) = self.ui_rasterizer {
            (0x20u8..=0x7Eu8).map(|code| {
                let ch = code as char;
                let key = GlyphKey::new_ui(ch, phys.font_size, false);
                let entry = self.glyph_atlas.get_or_insert(key, &mut **rast, phys.font_size);
                entry.advance
            }).collect()
        } else {
            Vec::new()
        }
    }

    /// Prepare GPU resources (atlas texture upload, vertex buffer) BEFORE the render pass.
    ///
    /// Renders both terminal grid content and UI chrome text through the
    /// same glyph atlas pipeline for consistent ClearType quality.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: Option<&RichGridData>,
        full_width: u32,
        full_height: u32,
        offset_x: f32,
        offset_y: f32,
        terminal_w: f32,
        terminal_h: f32,
        ui_text: &[TextCommand],
    ) {
        let mut vertices = if let Some(grid) = grid {
            atlas_vertex_builder::build_vertices(
                grid,
                &mut self.glyph_atlas,
                &self.font_metrics,
                &mut *self.rasterizer,
                self.default_fg,
                self.default_bg,
                full_width,
                full_height,
                offset_x,
                offset_y,
                terminal_w,
                terminal_h,
            )
        } else {
            Vec::new()
        };

        // Rasterize UI chrome text through the same atlas.
        // UI text uses transparent bg (alpha=0) to trigger grayscale AA mode
        // in the atlas shader, so text alpha-composites cleanly over gradient
        // and SDF-rounded backgrounds drawn by the quad pipeline.
        let phys = self.font_metrics.scaled_for_render();
        let vw = full_width as f32;
        let vh = full_height as f32;
        let transparent_bg = [0.0f32, 0.0, 0.0, 0.0];

        for cmd in ui_text {
            let mut cx = cmd.x;
            let use_ui_font = cmd.ui_font && self.ui_rasterizer.is_some();

            for ch in cmd.text.chars() {
                let (_key, entry) = if use_ui_font {
                    let key = GlyphKey::new_ui(ch, phys.font_size, cmd.bold);
                    let rast = self.ui_rasterizer.as_mut().unwrap();
                    let entry = self.glyph_atlas.get_or_insert(key, &mut **rast, phys.font_size);
                    (key, entry)
                } else {
                    let key = GlyphKey::new(ch, phys.font_size, cmd.bold, false);
                    let entry = self.glyph_atlas.get_or_insert(key, &mut *self.rasterizer, phys.font_size);
                    (key, entry)
                };

                let px = cx;
                let py = cmd.y;
                let sc = cmd.scale;
                // For proportional font, use actual glyph slot width from UV;
                // for monospace, use cell_width.
                let pw = if use_ui_font {
                    // Slot width = atlas slot pixel width from UV coords
                    (entry.u1 - entry.u0) * self.glyph_atlas.atlas_width() as f32 * sc
                } else {
                    phys.cell_width * sc
                };
                let ph = phys.cell_height * sc;

                let x0 = px / vw * 2.0 - 1.0;
                let y0 = 1.0 - py / vh * 2.0;
                let x1 = (px + pw) / vw * 2.0 - 1.0;
                let y1 = 1.0 - (py + ph) / vh * 2.0;

                // 6 vertices (2 triangles)
                let v = |pos: [f32; 2], uv: [f32; 2]| CellVertex {
                    position: pos,
                    uv,
                    fg_color: cmd.fg,
                    bg_color: transparent_bg,
                };
                let tl = [x0, y0];
                let br = [x1, y1];
                let tr = [br[0], tl[1]];
                let bl = [tl[0], br[1]];
                let uv_tl = [entry.u0, entry.v0];
                let uv_br = [entry.u1, entry.v1];
                let uv_tr = [uv_br[0], uv_tl[1]];
                let uv_bl = [uv_tl[0], uv_br[1]];

                vertices.push(v(tl, uv_tl));
                vertices.push(v(tr, uv_tr));
                vertices.push(v(bl, uv_bl));
                vertices.push(v(tr, uv_tr));
                vertices.push(v(bl, uv_bl));
                vertices.push(v(br, uv_br));

                // Advance by actual glyph advance for proportional, cell_width for mono
                cx += (if use_ui_font { entry.advance } else { phys.cell_width }) * sc;
            }
        }

        if vertices.is_empty() { return; }

        let atlas_update = self.glyph_atlas.take_dirty_data();
        let program = AtlasShaderProgram {
            vertices,
            atlas_update,
            viewport_size: (full_width, full_height),
        };
        let primitive = program.build_primitive();
        primitive.prepare(&mut self.pipeline, device, queue);
    }

    /// Draw the prepared terminal content into the render pass.
    pub fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        // AtlasPipeline stores vertex_count from prepare; draw uses the stored pipeline state
        // We need a dummy primitive to call draw - reuse the pipeline's stored vertex count
        let primitive = godly_terminal_surface::atlas_shader::AtlasShaderProgram {
            vertices: vec![],
            atlas_update: None,
            viewport_size: (0, 0),
        }.build_primitive();
        primitive.draw(&self.pipeline, render_pass);
    }

    pub fn font_metrics(&self) -> &FontMetrics {
        &self.font_metrics
    }
}
