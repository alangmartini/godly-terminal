use godly_terminal_surface::{
    atlas_shader::{AtlasPipeline, AtlasShaderProgram},
    atlas_vertex_builder,
    glyph_atlas::GlyphAtlas,
    glyph_rasterizer::GlyphRasterizer,
    font_metrics::FontMetrics,
    Color,
};
use godly_protocol::types::RichGridData;

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
            default_fg: Color::new(0.8, 0.8, 0.8, 1.0),
            default_bg: Color::new(0.07, 0.07, 0.10, 1.0),
        }
    }

    /// Prepare GPU resources (atlas texture upload, vertex buffer) BEFORE the render pass.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: &RichGridData,
        full_width: u32,
        full_height: u32,
        offset_x: f32,
        offset_y: f32,
    ) {
        let vertices = atlas_vertex_builder::build_vertices(
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
        );

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
