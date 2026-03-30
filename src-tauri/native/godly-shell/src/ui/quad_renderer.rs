//! Draws UI rectangles with SDF-based rounded corners, borders, and anti-aliasing.
//!
//! The shader uses a Signed Distance Field approach: each quad carries its rect
//! bounds, per-corner radii, and optional border/blur parameters.  The fragment
//! shader evaluates the SDF to produce smooth anti-aliased edges and crisp borders.
//!
//! Features:
//!   - Per-corner radius (vec4: TL, TR, BR, BL) for tab-style top-only rounding
//!   - Variable blur radius for soft box shadows
//!   - Vertical gradient via per-vertex fill_color interpolation
//!   - Flat quads (no radius/border) use a fast path that skips the SDF entirely

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub fill_color: [f32; 4],
    pub local_pos: [f32; 2],
    pub rect_half_ext: [f32; 2],
    pub corner_radii: [f32; 4],   // TL, TR, BR, BL
    pub border_width: f32,
    pub border_color: [f32; 4],
    pub blur_radius: f32,
}
// Total size: 8 + 16 + 8 + 8 + 16 + 4 + 16 + 4 = 80 bytes

impl QuadVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // position: clip-space xy
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            // fill_color: sRGB RGBA (interpolated for gradients)
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
            // corner_radii: TL, TR, BR, BL in pixels
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 40,
                shader_location: 4,
            },
            // border_width: pixels
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 56,
                shader_location: 5,
            },
            // border_color: sRGB RGBA
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 60,
                shader_location: 6,
            },
            // blur_radius: AA smoothstep width (0 = default 0.75px crisp)
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 76,
                shader_location: 7,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: 80,
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
    @location(4) corner_radii: vec4<f32>,
    @location(5) border_width: f32,
    @location(6) border_color: vec4<f32>,
    @location(7) blur_radius: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) @interpolate(flat) rect_half_ext: vec2<f32>,
    @location(3) @interpolate(flat) corner_radii: vec4<f32>,
    @location(4) @interpolate(flat) border_width: f32,
    @location(5) @interpolate(flat) border_color: vec4<f32>,
    @location(6) @interpolate(flat) blur_radius: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.fill_color = input.fill_color;
    out.local_pos = input.local_pos;
    out.rect_half_ext = input.rect_half_ext;
    out.corner_radii = input.corner_radii;
    out.border_width = input.border_width;
    out.border_color = input.border_color;
    out.blur_radius = input.blur_radius;
    return out;
}

// Signed distance to a rounded rectangle with per-corner radii.
// radii = (top_left, top_right, bottom_right, bottom_left)
// Returns negative inside, positive outside.
fn sd_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    // Select radius based on quadrant
    let r_top = select(radii.x, radii.y, p.x > 0.0);
    let r_bot = select(radii.w, radii.z, p.x > 0.0);
    let r = select(r_top, r_bot, p.y > 0.0);
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
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
    let d = sd_rounded_rect(input.local_pos, he, input.corner_radii);

    // AA band width: use blur_radius if set, otherwise 0.75px for crisp edges
    let blur = select(0.75, input.blur_radius, input.blur_radius > 0.0);
    let aa = 1.0 - smoothstep(-blur, blur, d);

    // Determine pixel color (fill or border)
    var color = fill;
    if (input.border_width > 0.0) {
        let inner_d = d + input.border_width;
        // Borders are always crisp (0.75px AA) regardless of blur
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
        corner_radii: [0.0; 4],
        border_width: 0.0,
        border_color: [0.0, 0.0, 0.0, 0.0],
        blur_radius: 0.0,
    };

    [
        v([x0, y0]), v([x1, y0]), v([x0, y1]),
        v([x0, y1]), v([x1, y0]), v([x1, y1]),
    ]
}

/// Build 6 vertices for a flat gradient rectangle (no SDF).
/// Top vertices get `top_color`, bottom vertices get `bottom_color`.
pub fn quad_vertices_gradient(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
) -> [QuadVertex; 6] {
    quad_vertices_gradient_dir(x, y, w, h, viewport_w, viewport_h, top_color, bottom_color, false)
}

/// Build 6 vertices for a flat horizontal gradient rectangle (no SDF).
/// Left vertices get `left_color`, right vertices get `right_color`.
pub fn quad_vertices_gradient_h(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    left_color: [f32; 4],
    right_color: [f32; 4],
) -> [QuadVertex; 6] {
    quad_vertices_gradient_dir(x, y, w, h, viewport_w, viewport_h, left_color, right_color, true)
}

/// Internal: vertical or horizontal gradient.
fn quad_vertices_gradient_dir(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    color_a: [f32; 4],
    color_b: [f32; 4],
    horizontal: bool,
) -> [QuadVertex; 6] {
    let x0 = x / viewport_w * 2.0 - 1.0;
    let y0 = -(y / viewport_h * 2.0 - 1.0);
    let x1 = (x + w) / viewport_w * 2.0 - 1.0;
    let y1 = -((y + h) / viewport_h * 2.0 - 1.0);

    let v = |pos: [f32; 2], color: [f32; 4]| QuadVertex {
        position: pos,
        fill_color: color,
        local_pos: [0.0, 0.0],
        rect_half_ext: [0.0, 0.0],
        corner_radii: [0.0; 4],
        border_width: 0.0,
        border_color: [0.0, 0.0, 0.0, 0.0],
        blur_radius: 0.0,
    };

    // Vertical gradient: top=color_a, bottom=color_b
    // Horizontal gradient: left=color_a, right=color_b
    let (tl, tr, bl, br) = if horizontal {
        (color_a, color_b, color_a, color_b)
    } else {
        (color_a, color_a, color_b, color_b)
    };

    [
        v([x0, y0], tl),
        v([x1, y0], tr),
        v([x0, y1], bl),
        v([x0, y1], bl),
        v([x1, y0], tr),
        v([x1, y1], br),
    ]
}

/// Build 6 vertices for an SDF rounded rectangle with optional border and blur.
///
/// `corner_radii` = [TL, TR, BR, BL] in pixels.
/// `blur_radius` controls the AA transition band (0 = default 0.75px crisp).
/// The geometry is expanded to accommodate AA/blur at the edges.
pub fn quad_vertices_sdf(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    fill_color: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    blur_radius: f32,
) -> [QuadVertex; 6] {
    let half_w = w / 2.0;
    let half_h = h / 2.0;
    let max_r = half_w.min(half_h);
    let radii = [
        corner_radii[0].min(max_r),
        corner_radii[1].min(max_r),
        corner_radii[2].min(max_r),
        corner_radii[3].min(max_r),
    ];

    // Expand geometry for AA (more expansion for blur/shadow)
    let pad = if blur_radius > 0.0 { blur_radius + 1.0 } else { 1.0 };
    let ex = x - pad;
    let ey = y - pad;
    let ew = w + pad * 2.0;
    let eh = h + pad * 2.0;

    // Clip-space positions (expanded rect)
    let x0 = ex / viewport_w * 2.0 - 1.0;
    let y0 = -(ey / viewport_h * 2.0 - 1.0);
    let x1 = (ex + ew) / viewport_w * 2.0 - 1.0;
    let y1 = -((ey + eh) / viewport_h * 2.0 - 1.0);

    // Local positions relative to rect center (including padding)
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
        corner_radii: radii,
        border_width,
        border_color,
        blur_radius,
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

/// Build 6 vertices for an SDF rounded rectangle with a vertical gradient.
/// Top vertices get `fill_color_top`, bottom get `fill_color_bottom`.
/// The GPU interpolates the color smoothly across the shape.
pub fn quad_vertices_sdf_gradient(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    fill_color_top: [f32; 4],
    fill_color_bottom: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
) -> [QuadVertex; 6] {
    let half_w = w / 2.0;
    let half_h = h / 2.0;
    let max_r = half_w.min(half_h);
    let radii = [
        corner_radii[0].min(max_r),
        corner_radii[1].min(max_r),
        corner_radii[2].min(max_r),
        corner_radii[3].min(max_r),
    ];

    let pad = 1.0;
    let ex = x - pad;
    let ey = y - pad;
    let ew = w + pad * 2.0;
    let eh = h + pad * 2.0;

    let x0 = ex / viewport_w * 2.0 - 1.0;
    let y0 = -(ey / viewport_h * 2.0 - 1.0);
    let x1 = (ex + ew) / viewport_w * 2.0 - 1.0;
    let y1 = -((ey + eh) / viewport_h * 2.0 - 1.0);

    let lx0 = -(half_w + pad);
    let ly0 = -(half_h + pad);
    let lx1 = half_w + pad;
    let ly1 = half_h + pad;

    let he = [half_w, half_h];

    let v = |pos: [f32; 2], lp: [f32; 2], color: [f32; 4]| QuadVertex {
        position: pos,
        fill_color: color,
        local_pos: lp,
        rect_half_ext: he,
        corner_radii: radii,
        border_width,
        border_color,
        blur_radius: 0.0,
    };

    [
        v([x0, y0], [lx0, ly0], fill_color_top),
        v([x1, y0], [lx1, ly0], fill_color_top),
        v([x0, y1], [lx0, ly1], fill_color_bottom),
        v([x0, y1], [lx0, ly1], fill_color_bottom),
        v([x1, y0], [lx1, ly0], fill_color_top),
        v([x1, y1], [lx1, ly1], fill_color_bottom),
    ]
}
