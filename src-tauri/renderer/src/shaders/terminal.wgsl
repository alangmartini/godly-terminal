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

@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(0) @binding(1)
var atlas_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    return output;
}

// Gamma correction exponent for stem darkening.
// pow(coverage, 1/gamma) boosts mid-range alpha, making text visually thicker
// and matching the weight produced by DirectWrite/ClearType renderers.
// 1.0/1.6 ≈ 0.625 — moderate darkening that matches Windows Terminal aesthetics.
const GAMMA_INV: f32 = 0.625;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let raw_alpha = textureSample(atlas_texture, atlas_sampler, input.uv).r;
    // Apply stem darkening: boost coverage for perceptually correct text weight
    let glyph_alpha = pow(raw_alpha, GAMMA_INV);
    return mix(input.bg_color, input.fg_color, glyph_alpha);
}
