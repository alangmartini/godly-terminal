//! GPU glyph atlas shader pipeline.
//!
//! Renders terminal content as per-cell textured quads sampling from a
//! persistent glyph atlas texture.  Each cell is 6 vertices (2 triangles)
//! carrying clip-space position, atlas UV, foreground and background colour.

use crate::atlas_vertex_builder::CellVertex;
use crate::glyph_atlas::AtlasUpdate;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Per-frame atlas render data stored on `TerminalInfo`.
#[derive(Debug, Clone)]
pub struct CachedAtlasFrame {
    pub vertices: Vec<CellVertex>,
    pub atlas_update: Option<AtlasUpdate>,
    pub viewport_size: (u32, u32),
}

/// Frame data carrier — call `build_primitive()` to produce an `AtlasPrimitive`.
pub struct AtlasShaderProgram {
    pub vertices: Vec<CellVertex>,
    pub atlas_update: Option<AtlasUpdate>,
    pub viewport_size: (u32, u32),
}

// ---------------------------------------------------------------------------
// WGSL — per-cell textured quads with alpha blending from atlas
// ---------------------------------------------------------------------------

const SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
};

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.fg_color = input.fg_color;
    out.bg_color = input.bg_color;
    return out;
}

// ClearType-style gamma blending with enhanced contrast.
//
// Windows ClearType uses gamma 1.8 (not sRGB 2.2) and an "enhanced contrast"
// parameter (default 0.5) that boosts mid-range coverage values to produce
// heavier, more readable text — especially on dark backgrounds.  These are
// the same parameters DirectWrite's IDWriteRenderingParams exposes.
const GAMMA: f32 = 1.8;
const INV_GAMMA: f32 = 0.5556; // 1.0 / 1.8
const ENHANCED_CONTRAST: f32 = 1.0;

// Boost coverage using DirectWrite's enhanced contrast formula.
fn enhance(c: f32) -> f32 {
    return clamp(c + ENHANCED_CONTRAST * c * (1.0 - c), 0.0, 1.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let raw = textureSample(atlas_tex, atlas_samp, input.uv);

    // Apply enhanced contrast to boost text weight.
    let coverage = vec3<f32>(enhance(raw.r), enhance(raw.g), enhance(raw.b));

    // Linearise fg/bg using ClearType gamma (1.8).
    let fg_lin = pow(input.fg_color.rgb, vec3<f32>(GAMMA));
    let bg_lin = pow(input.bg_color.rgb, vec3<f32>(GAMMA));

    // Per-channel subpixel blending in linear space.
    let blended = mix(bg_lin, fg_lin, coverage);

    // Back to gamma space.
    return vec4<f32>(pow(blended, vec3<f32>(INV_GAMMA)), 1.0);
}
"#;

// ---------------------------------------------------------------------------
// Pipeline (persistent GPU resources, stored in Iced's Storage)
// ---------------------------------------------------------------------------

pub struct AtlasPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    tex_format: wgpu::TextureFormat,
    atlas_texture: wgpu::Texture,
    #[allow(dead_code)]
    atlas_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    atlas_size: (u32, u32),
    atlas_generation: u64,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize, // in bytes
    vertex_count: u32,
}

impl AtlasPipeline {
    fn rgba_format(_compositor_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
        // Atlas stores coverage values (alpha), not colour data.
        // Always use linear (Unorm) so the GPU does not apply sRGB decode
        // which would distort coverage (making antialiasing too heavy).
        wgpu::TextureFormat::Rgba8Unorm
    }

    fn create_atlas_texture(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        tex_format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let w = width.max(1);
        let h = height.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas_texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: tex_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph_atlas_bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        (texture, view, bind_group)
    }
}

impl AtlasPipeline {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glyph_atlas_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glyph_atlas_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("glyph_atlas_pl"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("glyph_atlas_rp"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[CellVertex::layout()],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview: None,
                cache: None,
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let tex_format = Self::rgba_format(format);

        // 4×4 placeholder atlas — resized on first prepare().
        let (atlas_texture, atlas_view, bind_group) =
            Self::create_atlas_texture(device, &bind_group_layout, &sampler, 4, 4, tex_format);

        // Initial vertex buffer (64 KB, grows as needed).
        let vertex_capacity = 64 * 1024;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glyph_atlas_vb"),
            size: vertex_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            tex_format,
            atlas_texture,
            atlas_view,
            bind_group,
            atlas_size: (4, 4),
            atlas_generation: 0,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive (per-frame data, must be Send + 'static)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AtlasPrimitive {
    vertices: Vec<CellVertex>,
    atlas_update: Option<AtlasUpdate>,
}

impl AtlasPrimitive {
    pub fn prepare(
        &self,
        pipeline: &mut AtlasPipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // --- Atlas texture update ---
        if let Some(ref update) = self.atlas_update {
            if update.generation != pipeline.atlas_generation
                || (update.width, update.height) != pipeline.atlas_size
            {
                // Recreate atlas texture at new size.
                let (tex, view, bg) = AtlasPipeline::create_atlas_texture(
                    device,
                    &pipeline.bind_group_layout,
                    &pipeline.sampler,
                    update.width,
                    update.height,
                    pipeline.tex_format,
                );
                pipeline.atlas_texture = tex;
                pipeline.atlas_view = view;
                pipeline.bind_group = bg;
                pipeline.atlas_size = (update.width, update.height);
            }
            // Upload atlas pixel data.
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &pipeline.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &update.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(update.width * 4),
                    rows_per_image: Some(update.height),
                },
                wgpu::Extent3d {
                    width: update.width,
                    height: update.height,
                    depth_or_array_layers: 1,
                },
            );
            pipeline.atlas_generation = update.generation;
        }

        // --- Vertex buffer update ---
        let byte_data = bytemuck::cast_slice::<CellVertex, u8>(&self.vertices);
        let needed = byte_data.len();

        if needed > pipeline.vertex_capacity {
            // Grow vertex buffer (round up to next power of two).
            let new_cap = needed.next_power_of_two();
            pipeline.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("glyph_atlas_vb"),
                size: new_cap as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            pipeline.vertex_capacity = new_cap;
        }

        if !byte_data.is_empty() {
            queue.write_buffer(&pipeline.vertex_buffer, 0, byte_data);
        }
        pipeline.vertex_count = self.vertices.len() as u32;
    }

    pub fn draw(
        &self,
        pipeline: &AtlasPipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        if pipeline.vertex_count == 0 {
            return true;
        }
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.set_vertex_buffer(0, pipeline.vertex_buffer.slice(..));
        render_pass.draw(0..pipeline.vertex_count, 0..1);
        true
    }
}

impl AtlasShaderProgram {
    pub fn build_primitive(&self) -> AtlasPrimitive {
        AtlasPrimitive {
            vertices: self.vertices.clone(),
            atlas_update: self.atlas_update.clone(),
        }
    }
}
