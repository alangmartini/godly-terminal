//! Shader-based terminal rendering surface.
//!
//! Uses Iced's `Shader` widget with a persistent `wgpu::Texture` that is
//! updated in-place via `queue.write_texture()`.  This eliminates the
//! per-frame GPU texture churn caused by `Handle::from_rgba()` (which
//! creates a new Iced image handle ID on every call, forcing a full
//! texture upload -> swap -> deallocation cycle that produces visible
//! blinking).

use iced::widget::shader;
use iced::{mouse, Rectangle};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Cached pixel buffer stored on `TerminalInfo`.
#[derive(Debug, Clone)]
pub struct CachedPixelBuffer {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Per-frame pixel data carrier passed from `Program::draw()` to the GPU.
#[derive(Debug)]
pub struct TerminalPrimitive {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

/// Shader-based terminal surface.  Pass to `iced::widget::Shader::new()`.
pub struct TerminalShaderProgram {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// WGSL shader — fullscreen textured quad
// ---------------------------------------------------------------------------

const SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    let x = f32(idx & 1u) * 2.0 - 1.0;
    let y = f32((idx >> 1u) & 1u) * 2.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>(f32(idx & 1u), f32((idx >> 1u) & 1u));
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

// ---------------------------------------------------------------------------
// Pipeline — persistent GPU resources (created once, stored in Iced's Storage)
// ---------------------------------------------------------------------------

pub struct TerminalPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    tex_format: wgpu::TextureFormat,
    texture: wgpu::Texture,
    #[allow(dead_code)]
    texture_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    current_size: (u32, u32),
}

impl TerminalPipeline {
    /// Pick the RGBA texture format that matches Iced's compositor sRGB mode.
    /// Iced uses `Rgba8UnormSrgb` when `GAMMA_CORRECTION` is true (no
    /// `web-colors` feature) and `Rgba8Unorm` when false (`web-colors`
    /// enabled, which is the default).  Using the wrong variant causes a
    /// colour shift because the sRGB→linear conversion on texture read is
    /// not compensated by a matching linear→sRGB on framebuffer write.
    fn rgba_format(compositor_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
        if compositor_format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }

    fn create_texture(
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
            label: Some("terminal_shader_texture"),
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
            label: Some("terminal_shader_bg"),
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

impl shader::Pipeline for TerminalPipeline {
    fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terminal_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("terminal_shader_bgl"),
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
                label: Some("terminal_shader_pl"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("terminal_shader_rp"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
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
            label: Some("terminal_shader_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let tex_format = Self::rgba_format(format);

        // 1x1 placeholder — resized on first prepare().
        let (texture, texture_view, bind_group) =
            Self::create_texture(device, &bind_group_layout, &sampler, 1, 1, tex_format);

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            tex_format,
            texture,
            texture_view,
            bind_group,
            current_size: (1, 1),
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive impl — per-frame GPU work
// ---------------------------------------------------------------------------

impl shader::Primitive for TerminalPrimitive {
    type Pipeline = TerminalPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        let target = (self.width, self.height);
        if target != pipeline.current_size && self.width > 0 && self.height > 0 {
            let (tex, view, bg) = TerminalPipeline::create_texture(
                device,
                &pipeline.bind_group_layout,
                &pipeline.sampler,
                self.width,
                self.height,
                pipeline.tex_format,
            );
            pipeline.texture = tex;
            pipeline.texture_view = view;
            pipeline.bind_group = bg;
            pipeline.current_size = target;
        }

        if self.width > 0 && self.height > 0 && !self.pixels.is_empty() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &pipeline.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.width * 4),
                    rows_per_image: Some(self.height),
                },
                wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    fn draw(
        &self,
        pipeline: &Self::Pipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        if self.width == 0 || self.height == 0 {
            return true;
        }
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..4, 0..1);
        true
    }
}

// ---------------------------------------------------------------------------
// Program impl — Iced shader::Program for the Shader widget
// ---------------------------------------------------------------------------

impl<Message> shader::Program<Message> for TerminalShaderProgram {
    type State = ();
    type Primitive = TerminalPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        TerminalPrimitive {
            pixels: self.pixels.clone(),
            width: self.width,
            height: self.height,
        }
    }
}
