use godly_protocol::types::RichGridData;

use crate::atlas::GlyphAtlas;
use crate::color::resolve_cell_colors;
use crate::device::GpuDevice;
use crate::pipeline::{CellVertex, RenderPipeline};
use crate::theme::TerminalTheme;
use crate::GpuError;

/// GPU-accelerated terminal renderer.
///
/// Takes `RichGridData` snapshots from the daemon and renders them to
/// raw RGBA pixel buffers or PNG images using wgpu.
pub struct GpuRenderer {
    device: GpuDevice,
    atlas: GlyphAtlas,
    pipeline: RenderPipeline,
    theme: TerminalTheme,
}

/// Output texture format used for offscreen rendering.
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

impl GpuRenderer {
    /// Create a new GPU renderer with the given font family and size.
    ///
    /// Initializes the GPU device, glyph atlas, and render pipeline.
    /// Returns `GpuError::NoAdapter` if no GPU is available.
    pub fn new(font_family: &str, font_size: f32) -> Result<Self, GpuError> {
        let device = GpuDevice::new()?;
        let atlas = GlyphAtlas::new(font_family, font_size);
        let pipeline = RenderPipeline::new(&device.device, OUTPUT_FORMAT);
        let theme = TerminalTheme::default();

        Ok(Self {
            device,
            atlas,
            pipeline,
            theme,
        })
    }

    /// Set the terminal color theme.
    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.theme = theme;
    }

    /// Returns the current cell size (width, height) in pixels.
    pub fn cell_size(&self) -> (f32, f32) {
        self.atlas.cell_size()
    }

    /// Render a `RichGridData` snapshot to raw RGBA pixels.
    ///
    /// Returns `(width, height, rgba_pixels)` where `rgba_pixels` has
    /// length `width * height * 4`.
    pub fn render_to_pixels(
        &mut self,
        grid: &RichGridData,
    ) -> Result<(u32, u32, Vec<u8>), GpuError> {
        let (cell_w, cell_h) = self.atlas.cell_size();
        let width = (grid.dimensions.cols as f32 * cell_w).ceil() as u32;
        let height = (grid.dimensions.rows as f32 * cell_h).ceil() as u32;

        if width == 0 || height == 0 {
            return Ok((width, height, Vec::new()));
        }

        // 1. Build vertex data for all cells.
        let vertices = self.build_vertices(grid, width, height);

        if vertices.is_empty() {
            // No visible content -- return a solid background.
            let bg = self.theme.background;
            let r = (bg[0] * 255.0) as u8;
            let g = (bg[1] * 255.0) as u8;
            let b = (bg[2] * 255.0) as u8;
            let a = (bg[3] * 255.0) as u8;
            let mut pixels = vec![0u8; (width * height * 4) as usize];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[0] = r;
                pixel[1] = g;
                pixel[2] = b;
                pixel[3] = a;
            }
            return Ok((width, height, pixels));
        }

        let dev = &self.device.device;
        let queue = &self.device.queue;

        // 2. Upload vertex buffer.
        let vertex_data = bytemuck::cast_slice(&vertices);
        let vertex_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex_buffer"),
            size: vertex_data.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, vertex_data);

        // 3. Ensure glyph atlas texture is uploaded.
        let atlas_texture = self.atlas.ensure_gpu_texture(dev, queue);
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &self.pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // 4. Create output texture.
        let output_texture = dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("output_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 5. Record and submit render pass.
        let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        // Convert theme background to wgpu::Color (needs f64).
        let bg = self.theme.background;
        let clear_color = wgpu::Color {
            r: bg[0] as f64,
            g: bg[1] as f64,
            b: bg[2] as f64,
            a: bg[3] as f64,
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terminal_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }

        // 6. Copy output texture to staging buffer for CPU readback.
        // Row alignment: wgpu requires rows to be aligned to 256 bytes.
        let bytes_per_row = width * 4;
        let aligned_bytes_per_row = (bytes_per_row + 255) & !255;
        let staging_size = (aligned_bytes_per_row * height) as u64;

        let staging_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: staging_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // 7. Map staging buffer and read pixels.
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|e| GpuError::RenderError(format!("Buffer map channel error: {}", e)))?
            .map_err(|e| GpuError::RenderError(format!("Buffer map failed: {}", e)))?;

        let mapped = buffer_slice.get_mapped_range();

        // Remove row alignment padding.
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * aligned_bytes_per_row) as usize;
            let end = start + bytes_per_row as usize;
            pixels.extend_from_slice(&mapped[start..end]);
        }

        drop(mapped);
        staging_buffer.unmap();

        Ok((width, height, pixels))
    }

    /// Render a `RichGridData` snapshot to PNG bytes.
    pub fn render_to_png(&mut self, grid: &RichGridData) -> Result<Vec<u8>, GpuError> {
        let (width, height, pixels) = self.render_to_pixels(grid)?;
        if width == 0 || height == 0 {
            return Err(GpuError::RenderError("Empty grid".to_string()));
        }

        let img = image::RgbaImage::from_raw(width, height, pixels)
            .ok_or(GpuError::ImageError)?;
        let mut png_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| GpuError::RenderError(format!("PNG encode error: {}", e)))?;
        Ok(png_bytes)
    }

    /// Build vertex data for all cells in the grid.
    ///
    /// Each non-continuation cell produces 6 vertices (2 triangles).
    /// Positions are in clip space (-1..1). UVs reference the glyph atlas.
    fn build_vertices(
        &mut self,
        grid: &RichGridData,
        width: u32,
        height: u32,
    ) -> Vec<CellVertex> {
        let (cell_w, cell_h) = self.atlas.cell_size();
        let capacity = grid.dimensions.rows as usize * grid.dimensions.cols as usize * 6;
        let mut vertices = Vec::with_capacity(capacity);
        let w_f = width as f32;
        let h_f = height as f32;

        for (row_idx, row) in grid.rows.iter().enumerate() {
            for (col_idx, cell) in row.cells.iter().enumerate() {
                // Skip wide continuation cells (drawn by the wide-start cell).
                if cell.wide_continuation {
                    continue;
                }

                // Pixel position of cell top-left corner.
                let x = col_idx as f32 * cell_w;
                let y = row_idx as f32 * cell_h;
                let w = if cell.wide { cell_w * 2.0 } else { cell_w };
                let h = cell_h;

                // Resolve foreground/background colors.
                let (mut fg, bg) = resolve_cell_colors(
                    &cell.fg,
                    &cell.bg,
                    cell.inverse,
                    self.theme.foreground,
                    self.theme.background,
                );

                // Apply dim attribute: halve foreground brightness.
                if cell.dim {
                    fg = [fg[0] * 0.5, fg[1] * 0.5, fg[2] * 0.5, fg[3]];
                }

                // Get glyph atlas entry.
                let ch = cell.content.chars().next().unwrap_or(' ');
                let entry = self.atlas.get_or_insert(ch, cell.bold, cell.italic);

                // Convert pixel coords to clip space: x: -1..1, y: 1..-1 (top to bottom).
                let x0 = (x / w_f) * 2.0 - 1.0;
                let y0 = 1.0 - (y / h_f) * 2.0;
                let x1 = ((x + w) / w_f) * 2.0 - 1.0;
                let y1 = 1.0 - ((y + h) / h_f) * 2.0;

                // Two triangles per cell (6 vertices).
                // Triangle 1: top-left, top-right, bottom-left
                // Triangle 2: bottom-left, top-right, bottom-right
                vertices.extend_from_slice(&[
                    CellVertex {
                        position: [x0, y0],
                        uv: [entry.u0, entry.v0],
                        fg_color: fg,
                        bg_color: bg,
                    },
                    CellVertex {
                        position: [x1, y0],
                        uv: [entry.u1, entry.v0],
                        fg_color: fg,
                        bg_color: bg,
                    },
                    CellVertex {
                        position: [x0, y1],
                        uv: [entry.u0, entry.v1],
                        fg_color: fg,
                        bg_color: bg,
                    },
                    CellVertex {
                        position: [x0, y1],
                        uv: [entry.u0, entry.v1],
                        fg_color: fg,
                        bg_color: bg,
                    },
                    CellVertex {
                        position: [x1, y0],
                        uv: [entry.u1, entry.v0],
                        fg_color: fg,
                        bg_color: bg,
                    },
                    CellVertex {
                        position: [x1, y1],
                        uv: [entry.u1, entry.v1],
                        fg_color: fg,
                        bg_color: bg,
                    },
                ]);

                // Add underline quad if the cell is underlined.
                if cell.underline {
                    let underline_h = (cell_h * 0.06).max(1.0); // 1px minimum
                    let uy0 = 1.0 - ((y + h - underline_h) / h_f) * 2.0;
                    let uy1 = 1.0 - ((y + h) / h_f) * 2.0;
                    let blank = self.atlas.get_or_insert(' ', false, false);
                    // Solid fg-colored quad (blank glyph -> mix returns bg, so use fg as both)
                    vertices.extend_from_slice(&[
                        CellVertex { position: [x0, uy0], uv: [blank.u0, blank.v0], fg_color: fg, bg_color: fg },
                        CellVertex { position: [x1, uy0], uv: [blank.u1, blank.v0], fg_color: fg, bg_color: fg },
                        CellVertex { position: [x0, uy1], uv: [blank.u0, blank.v1], fg_color: fg, bg_color: fg },
                        CellVertex { position: [x0, uy1], uv: [blank.u0, blank.v1], fg_color: fg, bg_color: fg },
                        CellVertex { position: [x1, uy0], uv: [blank.u1, blank.v0], fg_color: fg, bg_color: fg },
                        CellVertex { position: [x1, uy1], uv: [blank.u1, blank.v1], fg_color: fg, bg_color: fg },
                    ]);
                }
            }
        }

        // Add cursor overlay if visible.
        if !grid.cursor_hidden {
            let cursor_row = grid.cursor.row as usize;
            let cursor_col = grid.cursor.col as usize;

            let x = cursor_col as f32 * cell_w;
            let y = cursor_row as f32 * cell_h;

            let x0 = (x / w_f) * 2.0 - 1.0;
            let y0 = 1.0 - (y / h_f) * 2.0;
            let x1 = ((x + cell_w) / w_f) * 2.0 - 1.0;
            let y1 = 1.0 - ((y + cell_h) / h_f) * 2.0;

            let cursor_fg = self.theme.cursor_color;
            // Use cursor color as bg so it shows as a solid block.
            // The glyph UV points to blank (space) so alpha=0, meaning
            // mix() returns bg_color = cursor_color.
            let blank = self.atlas.get_or_insert(' ', false, false);

            vertices.extend_from_slice(&[
                CellVertex {
                    position: [x0, y0],
                    uv: [blank.u0, blank.v0],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
                CellVertex {
                    position: [x1, y0],
                    uv: [blank.u1, blank.v0],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
                CellVertex {
                    position: [x0, y1],
                    uv: [blank.u0, blank.v1],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
                CellVertex {
                    position: [x0, y1],
                    uv: [blank.u0, blank.v1],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
                CellVertex {
                    position: [x1, y0],
                    uv: [blank.u1, blank.v0],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
                CellVertex {
                    position: [x1, y1],
                    uv: [blank.u1, blank.v1],
                    fg_color: cursor_fg,
                    bg_color: cursor_fg,
                },
            ]);
        }

        vertices
    }
}
