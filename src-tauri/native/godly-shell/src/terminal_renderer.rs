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

    /// Render a terminal grid.
    ///
    /// `full_width`/`full_height` = total window size (for clip-space mapping).
    /// `offset_x`/`offset_y` = pixel offset of the terminal area within the window.
    /// `area_width`/`area_height` = size of the terminal area (for grid sizing).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        grid: &RichGridData,
        full_width: u32,
        full_height: u32,
        offset_x: f32,
        offset_y: f32,
    ) -> usize {
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

        if vertices.is_empty() {
            log::debug!("build_vertices returned empty");
            return 0;
        }
        log::debug!("Rendering {} vertices at offset ({}, {}), viewport {}x{}",
            vertices.len(), offset_x, offset_y, full_width, full_height);

        let atlas_update = self.glyph_atlas.take_dirty_data();
        let program = AtlasShaderProgram {
            vertices,
            atlas_update,
            viewport_size: (full_width, full_height),
        };
        let primitive = program.build_primitive();
        primitive.prepare(&mut self.pipeline, device, queue);
        primitive.draw(&self.pipeline, render_pass);

        grid.dimensions.rows as usize * grid.dimensions.cols as usize
    }

    pub fn font_metrics(&self) -> &FontMetrics {
        &self.font_metrics
    }
}
