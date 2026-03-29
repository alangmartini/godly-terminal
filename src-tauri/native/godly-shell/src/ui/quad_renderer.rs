//! Draws solid-color rectangles for UI chrome (title bar, tab backgrounds, etc.).

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl QuadVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 8,
                shader_location: 1,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

const SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
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

        let vertex_capacity = 4096;
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

/// Build 6 vertices (2 triangles) for a colored rectangle.
pub fn quad_vertices(
    x: f32, y: f32, w: f32, h: f32,
    viewport_w: f32, viewport_h: f32,
    color: [f32; 4],
) -> [QuadVertex; 6] {
    // Convert pixel coords to clip space (-1..1)
    let x0 = x / viewport_w * 2.0 - 1.0;
    let y0 = -(y / viewport_h * 2.0 - 1.0);
    let x1 = (x + w) / viewport_w * 2.0 - 1.0;
    let y1 = -((y + h) / viewport_h * 2.0 - 1.0);

    [
        QuadVertex { position: [x0, y0], color },
        QuadVertex { position: [x1, y0], color },
        QuadVertex { position: [x0, y1], color },
        QuadVertex { position: [x0, y1], color },
        QuadVertex { position: [x1, y0], color },
        QuadVertex { position: [x1, y1], color },
    ]
}
