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
    font_metrics: FontMetrics,
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
            font_metrics,
            default_fg: Color::new(0.671, 0.698, 0.749, 1.0),  // One Dark Text #abb2bf
            default_bg: Color::new(0.129, 0.145, 0.169, 1.0), // One Dark Base #21252b
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
            for ch in cmd.text.chars() {
                let key = GlyphKey::new(ch, phys.font_size, false, false);
                let entry = self.glyph_atlas.get_or_insert(key, &mut *self.rasterizer, phys.font_size);

                let px = cx;
                let py = cmd.y;
                let pw = phys.cell_width;
                let ph = phys.cell_height;

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

                cx += pw;
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
