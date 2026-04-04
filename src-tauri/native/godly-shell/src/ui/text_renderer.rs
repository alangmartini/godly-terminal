//! Simple UI text rendering using the glyph atlas pipeline.

use godly_terminal_surface::{
    atlas_shader::{AtlasPipeline, AtlasShaderProgram, TextRenderParams},
    atlas_vertex_builder::CellVertex,
    glyph_atlas::GlyphAtlas,
    glyph_cache::GlyphKey,
    glyph_rasterizer::GlyphRasterizer,
};

/// Renders UI text (title bar labels, button symbols, sidebar items).
pub struct UiTextRenderer {
    pipeline: AtlasPipeline,
    atlas: GlyphAtlas,
    rasterizer: Box<dyn GlyphRasterizer>,
    cell_w: f32,
    cell_h: f32,
    font_size: f32,
}

impl UiTextRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cell_w: f32,
        cell_h: f32,
        baseline: f32,
        font_size: f32,
        rasterizer: Box<dyn GlyphRasterizer>,
    ) -> Self {
        let pipeline = AtlasPipeline::new(device, queue, format);
        // Match terminal renderer's coverage attenuation for consistent text weight.
        let params = TextRenderParams {
            coverage_attenuation: 0.92,
            ..TextRenderParams::default()
        };
        pipeline.set_text_render_params(queue, &params);
        let atlas = GlyphAtlas::new(cell_w, cell_h, baseline);
        Self { pipeline, atlas, rasterizer, cell_w, cell_h, font_size }
    }

    /// Build vertices for a text string at pixel position (px, py).
    /// Returns vertices in clip-space for viewport (vw, vh).
    pub fn build_text_vertices(
        &mut self,
        text: &str,
        px: f32,
        py: f32,
        vw: f32,
        vh: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) -> Vec<CellVertex> {
        let mut verts = Vec::new();
        for (i, ch) in text.chars().enumerate() {
            let x = px + i as f32 * self.cell_w;
            let y = py;

            let key = GlyphKey::new(ch, self.font_size, false, false);
            let entry = self.atlas.get_or_insert(key, &mut *self.rasterizer, self.font_size);

            let x0 = x / vw * 2.0 - 1.0;
            let y0 = 1.0 - y / vh * 2.0;
            let x1 = (x + self.cell_w) / vw * 2.0 - 1.0;
            let y1 = 1.0 - (y + self.cell_h) / vh * 2.0;

            // TL, TR, BL
            verts.push(CellVertex { position: [x0, y0], uv: [entry.u0, entry.v0], fg_color: fg, bg_color: bg });
            verts.push(CellVertex { position: [x1, y0], uv: [entry.u1, entry.v0], fg_color: fg, bg_color: bg });
            verts.push(CellVertex { position: [x0, y1], uv: [entry.u0, entry.v1], fg_color: fg, bg_color: bg });
            // TR, BL, BR
            verts.push(CellVertex { position: [x1, y0], uv: [entry.u1, entry.v0], fg_color: fg, bg_color: bg });
            verts.push(CellVertex { position: [x0, y1], uv: [entry.u0, entry.v1], fg_color: fg, bg_color: bg });
            verts.push(CellVertex { position: [x1, y1], uv: [entry.u1, entry.v1], fg_color: fg, bg_color: bg });
        }
        verts
    }

    /// Prepare and draw text in a single call (call before render pass for prepare, during for draw).
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: Vec<CellVertex>,
        viewport: (u32, u32),
    ) {
        if vertices.is_empty() { return; }
        let atlas_update = self.atlas.take_dirty_data();
        let program = AtlasShaderProgram {
            vertices,
            atlas_update,
            viewport_size: viewport,
        };
        let primitive = program.build_primitive();
        primitive.prepare(&mut self.pipeline, device, queue);
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        let primitive = AtlasShaderProgram {
            vertices: vec![],
            atlas_update: None,
            viewport_size: (0, 0),
        }.build_primitive();
        primitive.draw(&self.pipeline, render_pass);
    }
}
