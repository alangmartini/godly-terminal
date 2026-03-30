//! Draws UI rectangles with SDF-based rounded corners, borders, and anti-aliasing.
//!
//! The shader uses a Signed Distance Field approach: each quad carries its rect
//! bounds, corner radius, and optional border parameters.  The fragment shader
//! evaluates the SDF to produce smooth anti-aliased edges and crisp borders.
//!
//! Flat quads (no radius/border) use a fast path that skips the SDF entirely.

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub fill_color: [f32; 4],
    pub local_pos: [f32; 2],
    pub rect_half_ext: [f32; 2],
    pub corner_radius: f32,
    pub border_width: f32,
    pub border_color: [f32; 4],
}

impl QuadVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // position: clip-space xy
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            // fill_color: sRGB RGBA
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
            // local_pos: pixel-space position relative to rect center
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            // rect_half_ext: half-extents of the rect in pixels
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 32,
                shader_location: 3,
            },
            // corner_radius: pixels
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 40,
                shader_location: 4,
            },
            // border_width: pixels
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 44,
                shader_location: 5,
            },
            // border_color: sRGB RGBA
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 6,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

const SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
    @location(2) local_pos: vec2<f32>,
    @location(3) rect_half_ext: vec2<f32>,
    @location(4) corner_radius: f32,
    @location(5) border_width: f32,
    @location(6) border_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) @interpolate(flat) rect_half_ext: vec2<f32>,
    @location(3) @interpolate(flat) corner_radius: f32,
    @location(4) @interpolate(flat) border_width: f32,
    @location(5) @interpolate(flat) border_color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.fill_color = input.fill_color;
    out.local_pos = input.local_pos;
    out.rect_half_ext = input.rect_half_ext;
    out.corner_radius = input.corner_radius;
    out.border_width = input.border_width;
    out.border_color = input.border_color;
    return out;
}

// Signed distance to a rounded rectangle centered at the origin.
// Returns negative inside, positive outside.
fn sd_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let fill = input.fill_color;
    let he = input.rect_half_ext;

    // Fast path: flat quads with no SDF (rect_half_ext.x <= 0 signals flat mode)
    if (he.x <= 0.0) {
        return vec4<f32>(pow(fill.rgb, vec3<f32>(2.2)), fill.a);
    }

    // SDF path: rounded rectangle with anti-aliased edges
    let d = sd_rounded_rect(input.local_pos, he, input.corner_radius);

    // Anti-aliased alpha at the outer edge (~1.5px transition band)
    let aa = 1.0 - smoothstep(-0.75, 0.75, d);

    // Determine pixel color (fill or border)
    var color = fill;
    if (input.border_width > 0.0) {
        let inner_d = d + input.border_width;
        let fill_mask = 1.0 - smoothstep(-0.75, 0.75, inner_d);
        color = vec4<f32>(
            mix(input.border_color.rgb, fill.rgb, fill_mask),
            mix(input.border_color.a, fill.a, fill_mask),
        );
    }

    // Convert sRGB to linear + apply AA alpha
    return vec4<f32>(pow(color.rgb, vec3<f32>(2.2)), color.a * aa);
}
"#;

pub struct QuadPipeline {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    vertex_count: u32,
}

impl QuadPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad_pl"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad_rp"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::layout()],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let vertex_capacity = 65536;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad_vb"),
            size: vertex_capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
        }
    }

    /// Upload quad vertices and draw them.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        vertices: &[QuadVertex],
    ) {
        if vertices.is_empty() { return; }

        let byte_data = bytemuck::cast_slice::<QuadVertex, u8>(vertices);
        if byte_data.len() > self.vertex_capacity {
            let new_cap = byte_data.len().next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("quad_vb"),
                size: new_cap as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_cap;
        }

        queue.write_buffer(&self.vertex_buffer, 0, byte_data);
        self.vertex_count = vertices.len() as u32;

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Vertex construction helpers
// ---------------------------------------------------------------------------

/// Build 6 vertices for a flat solid-color rectangle (no SDF, no rounding).
pub fn quad_vertices(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    color: [f32; 4],
) -> [QuadVertex; 6] {
    let x0 = x / viewport_w * 2.0 - 1.0;
    let y0 = -(y / viewport_h * 2.0 - 1.0);
    let x1 = (x + w) / viewport_w * 2.0 - 1.0;
    let y1 = -((y + h) / viewport_h * 2.0 - 1.0);

    let v = |pos: [f32; 2]| QuadVertex {
        position: pos,
        fill_color: color,
        local_pos: [0.0, 0.0],
        rect_half_ext: [0.0, 0.0], // signals flat path in shader
        corner_radius: 0.0,
        border_width: 0.0,
        border_color: [0.0, 0.0, 0.0, 0.0],
    };

    [
        v([x0, y0]), v([x1, y0]), v([x0, y1]),
        v([x0, y1]), v([x1, y0]), v([x1, y1]),
    ]
}

/// Build 6 vertices for an SDF rounded rectangle with optional border.
///
/// The geometry is expanded by 1px on each side so the SDF anti-aliasing
/// has room to fade to transparent at the edges.
pub fn quad_vertices_sdf(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    fill_color: [f32; 4],
    corner_radius: f32,
    border_width: f32,
    border_color: [f32; 4],
) -> [QuadVertex; 6] {
    let half_w = w / 2.0;
    let half_h = h / 2.0;
    let r = corner_radius.min(half_w).min(half_h);

    // Expand geometry by 1px for AA
    let pad = 1.0;
    let ex = x - pad;
    let ey = y - pad;
    let ew = w + pad * 2.0;
    let eh = h + pad * 2.0;

    // Clip-space positions (expanded rect)
    let x0 = ex / viewport_w * 2.0 - 1.0;
    let y0 = -(ey / viewport_h * 2.0 - 1.0);
    let x1 = (ex + ew) / viewport_w * 2.0 - 1.0;
    let y1 = -((ey + eh) / viewport_h * 2.0 - 1.0);

    // Local positions relative to rect center (including AA padding)
    let lx0 = -(half_w + pad);
    let ly0 = -(half_h + pad);
    let lx1 = half_w + pad;
    let ly1 = half_h + pad;

    let he = [half_w, half_h];

    let v = |pos: [f32; 2], lp: [f32; 2]| QuadVertex {
        position: pos,
        fill_color,
        local_pos: lp,
        rect_half_ext: he,
        corner_radius: r,
        border_width,
        border_color,
    };

    [
        v([x0, y0], [lx0, ly0]),
        v([x1, y0], [lx1, ly0]),
        v([x0, y1], [lx0, ly1]),
        v([x0, y1], [lx0, ly1]),
        v([x1, y0], [lx1, ly0]),
        v([x1, y1], [lx1, ly1]),
    ]
}
