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
//!   - Rotation support for angled SDF shapes (used by icon rendering)
//!   - Premultiplied alpha blending for correct multi-layer compositing
//!   - Flat quads (no radius/border) use a fast path that skips the SDF entirely

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
    pub fill_color: [f32; 4],
    pub local_pos: [f32; 2],
    pub rect_half_ext: [f32; 2],
    pub corner_radii: [f32; 4], // TL, TR, BR, BL
    pub border_widths: [f32; 4], // top, right, bottom, left (CSS order)
    pub border_color: [f32; 4],
    pub blur_radius: f32,
    pub rotation: f32,
    pub lighting_intensity: f32,
    pub clip_rect: [f32; 4], // x_min, y_min, x_max, y_max in screen pixels
    pub corner_smoothness: f32, // 0.0 = circular (CSS), 1.0 = full squircle (Apple/iOS)
    pub gradient_color_mid: [f32; 4], // Middle stop color (sRGB RGBA) for multi-stop gradients
    pub gradient_config: [f32; 4], // .x=stop_count (0=off, 3=three-stop), .y=direction (0=horiz, 1=vert), .z=mid_t, .w=reserved
}
// Total size: 8 + 16 + 8 + 8 + 16 + 16 + 16 + 4 + 4 + 4 + 16 + 4 + 16 + 16 = 152 bytes

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
            // border_widths: top, right, bottom, left in pixels (CSS order)
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 56,
                shader_location: 5,
            },
            // border_color: sRGB RGBA
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 72,
                shader_location: 6,
            },
            // blur_radius: AA smoothstep width (0 = default 0.75px crisp)
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 88,
                shader_location: 7,
            },
            // rotation: radians (0 = no rotation)
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 92,
                shader_location: 8,
            },
            // lighting_intensity: 0.0 = flat CSS-like, 1.0 = full 3D lighting
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 96,
                shader_location: 9,
            },
            // clip_rect: screen-pixel clip bounds (x_min, y_min, x_max, y_max)
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 100,
                shader_location: 10,
            },
            // corner_smoothness: 0.0 = circular, 1.0 = full squircle
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 116,
                shader_location: 11,
            },
            // gradient_color_mid: sRGB RGBA middle stop for multi-stop gradients
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 120,
                shader_location: 12,
            },
            // gradient_config: stop_count, direction, mid_t, reserved
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 136,
                shader_location: 13,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: 152,
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
    @location(5) border_widths: vec4<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) blur_radius: f32,
    @location(8) rotation: f32,
    @location(9) lighting_intensity: f32,
    @location(10) clip_rect: vec4<f32>,
    @location(11) corner_smoothness: f32,
    @location(12) gradient_color_mid: vec4<f32>,
    @location(13) gradient_config: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) @interpolate(flat) rect_half_ext: vec2<f32>,
    @location(3) @interpolate(flat) corner_radii: vec4<f32>,
    @location(4) @interpolate(flat) border_widths: vec4<f32>,
    @location(5) @interpolate(flat) border_color: vec4<f32>,
    @location(6) @interpolate(flat) blur_radius: f32,
    @location(7) @interpolate(flat) rotation: f32,
    @location(8) @interpolate(flat) lighting_intensity: f32,
    @location(9) @interpolate(flat) clip_rect: vec4<f32>,
    @location(10) @interpolate(flat) corner_smoothness: f32,
    @location(11) @interpolate(flat) gradient_color_mid: vec4<f32>,
    @location(12) @interpolate(flat) gradient_config: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    // Convert fill_color from sRGB to linear so GPU hardware interpolation
    // (for gradients) happens in perceptually correct linear space.
    out.fill_color = vec4<f32>(srgb_to_linear(input.fill_color.rgb), input.fill_color.a);
    out.local_pos = input.local_pos;
    out.rect_half_ext = input.rect_half_ext;
    out.corner_radii = input.corner_radii;
    out.border_widths = input.border_widths;
    out.border_color = input.border_color;
    out.blur_radius = input.blur_radius;
    out.rotation = input.rotation;
    out.lighting_intensity = input.lighting_intensity;
    out.clip_rect = input.clip_rect;
    out.corner_smoothness = input.corner_smoothness;
    // Linearize gradient mid color the same way as fill_color for correct interpolation
    out.gradient_color_mid = vec4<f32>(srgb_to_linear(input.gradient_color_mid.rgb), input.gradient_color_mid.a);
    out.gradient_config = input.gradient_config;
    return out;
}

// sRGB ↔ linear conversion (IEC 61966-2-1 spec-correct piecewise transfer).
// Vertex colors are specified in sRGB space (matching CSS hex/rgb values).
// The GPU interpolates vertex outputs linearly, so we convert to linear in
// the vertex shader — the GPU then interpolates in linear space — and convert
// back to sRGB in the fragment shader before dither/lighting.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let s = max(c, vec3<f32>(0.0));
    let lo = s / vec3<f32>(12.92);
    let hi = pow((s + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(hi, lo, s <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let s = max(c, vec3<f32>(0.0));
    let lo = s * vec3<f32>(12.92);
    let hi = vec3<f32>(1.055) * pow(s, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, s <= vec3<f32>(0.0031308));
}

// Signed distance to a rounded rectangle with per-corner radii and optional
// superellipse (squircle) corners.
// radii = (top_left, top_right, bottom_right, bottom_left)
// smoothness: 0.0 = circular (CSS border-radius), 1.0 = full squircle (Apple/iOS)
// Returns negative inside, positive outside.
fn sd_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>, smoothness: f32) -> f32 {
    // Select radius based on quadrant
    let r_top = select(radii.x, radii.y, p.x > 0.0);
    let r_bot = select(radii.w, radii.z, p.x > 0.0);
    let r = select(r_top, r_bot, p.y > 0.0);
    let q = abs(p) - half_size + vec2<f32>(r);

    // Lp norm: p=2 is circular (L2), p=5 is full squircle.
    // The Lp norm produces continuous-curvature corners that transition
    // smoothly from straight edges — the same approach Apple uses in iOS
    // and Figma uses for "corner smoothing".
    let n = 2.0 + smoothness * 3.0;
    let qx = max(q.x, 0.0);
    let qy = max(q.y, 0.0);
    // When smoothness is 0 (n=2), this is equivalent to length(max(q,0))
    let outer = pow(pow(qx, n) + pow(qy, n), 1.0 / n);
    return outer + min(max(q.x, q.y), 0.0) - r;
}

// Triangle-distributed noise for gradient dithering.
// Eliminates visible color banding on dark gradients (8-bit displays).
fn dither_noise(pos: vec2<f32>) -> f32 {
    // Two independent hash values combined into triangle distribution
    let n1 = fract(sin(dot(pos, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let n2 = fract(sin(dot(pos, vec2<f32>(39.346, 11.135))) * 43758.5453);
    return (n1 + n2 - 1.0) / 255.0;  // ±1 LSB triangle noise
}

// Spatially-coherent hash noise for material micro-texture.
// Unlike dither_noise (which varies per-frame for banding), this produces
// a stable, screen-position-keyed grain that gives surfaces a subtle
// "brushed matte" quality — the difference between a flat digital rectangle
// and a real material surface.  Quantized to 2×2 pixel blocks for a fine
// but visible texture at typical DPI, with no temporal flicker.
//
// Intensity 0.004 (~±1 LSB in sRGB) is barely perceptible on smooth
// surfaces but prevents the uncanny "perfectly digital" flatness.
// Higher values (e.g. 0.006) created visible speckle on dark backgrounds.
fn material_grain(pos: vec2<f32>) -> f32 {
    let p = floor(pos * 0.5);
    return (fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453) - 0.5) * 0.004;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Convert interpolated linear fill_color back to sRGB.  The vertex shader
    // linearized it so the GPU's hardware interpolation (for gradients) happens
    // in perceptually correct linear space.  The rest of the fragment shader
    // expects sRGB for dither, lighting, and the final srgb_to_linear() output path.
    var fill = vec4<f32>(linear_to_srgb(input.fill_color.rgb), input.fill_color.a);
    let he = input.rect_half_ext;
    let screen_pos = input.position.xy;

    // Clip rectangle: discard fragments outside the clip bounds with smooth AA.
    // clip_rect = (x_min, y_min, x_max, y_max) in screen pixels.
    // Default [0, 0, 99999, 99999] means no clipping (z > 50000 sentinel check).
    var clip_alpha = 1.0;
    let cr = input.clip_rect;
    if (cr.z < 50000.0) {
        let clip_dx = max(cr.x - screen_pos.x, screen_pos.x - cr.z);
        let clip_dy = max(cr.y - screen_pos.y, screen_pos.y - cr.w);
        let clip_d = max(clip_dx, clip_dy);
        if (clip_d > 1.0) {
            discard;
        }
        // 1px feather for anti-aliased clip edges
        clip_alpha = 1.0 - smoothstep(-1.0, 1.0, clip_d);
    }

    // Fast path: flat quads with no SDF (rect_half_ext.x <= 0 signals flat mode)
    if (he.x <= 0.0) {
        // CSS renders backgrounds as perfectly flat solid colors — no dither
        // or grain.  Keeping these flat for pixel-exact web parity.
        let linear = srgb_to_linear(fill.rgb);
        // Premultiplied alpha output: RGB * alpha before blending.
        // With PREMULTIPLIED_ALPHA_BLENDING, this produces correct compositing
        // of semi-transparent layers (shadows, glows, hover transitions) and
        // eliminates dark fringes at anti-aliased edges of colored elements.
        let flat_a = fill.a * clip_alpha;
        return vec4<f32>(linear * flat_a, flat_a);
    }

    // Rotate local_pos into shape space when rotation is non-zero
    var p = input.local_pos;
    if (input.rotation != 0.0) {
        let cos_r = cos(input.rotation);
        let sin_r = sin(input.rotation);
        p = vec2<f32>(
            p.x * cos_r + p.y * sin_r,
            -p.x * sin_r + p.y * cos_r,
        );
    }

    // SDF path: rounded rectangle with anti-aliased edges
    let d = sd_rounded_rect(p, he, input.corner_radii, input.corner_smoothness);

    // Screen-space adaptive AA: fwidth(d) gives the rate of change of the SDF
    // distance across the pixel.  Using this instead of a fixed pixel width
    // produces perfect anti-aliasing at any DPI / zoom level — thin crisp edges
    // on 4K, smooth on 1080p, no manual tuning needed.
    let fw = fwidth(d);
    let crisp_aa = fw * 0.75;

    // AA / shadow alpha
    // Negative blur_radius signals INNER shadow mode: shadow falls inside the
    // shape, fading inward from the edges.  Positive blur = outer shadow/glow.
    let is_inner_shadow = input.blur_radius < -0.5;
    let abs_blur = abs(input.blur_radius);
    let blur = select(crisp_aa, abs_blur, abs_blur > 0.0);
    var aa: f32;
    if (is_inner_shadow) {
        // Inner shadow: Gaussian falloff from the inner edge of the shape.
        // Shadow is strongest near the edge (d ≈ 0) and fades toward the
        // center (d << 0) — the inverse of outer shadow behavior.
        let sigma = blur * 0.45;
        let inner_d = max(-d, 0.0);  // 0 at edge, grows toward center
        let gauss = exp(-0.5 * (inner_d * inner_d) / (sigma * sigma));
        // Anti-aliased boundary: smooth transition at the shape edge so
        // the inner shadow doesn't have a hard cut at d=0.
        let edge_aa = smoothstep(crisp_aa, -crisp_aa, d);
        aa = gauss * edge_aa;
    } else if (blur > 2.0) {
        // Gaussian falloff for soft shadows/glows — more natural than smoothstep.
        // Produces a brighter core with gentle exponential fade at the edges,
        // mimicking real light scattering from emissive surfaces.
        let sigma = blur * 0.45;
        let clamped_d = max(d, 0.0);
        let gauss = exp(-0.5 * (clamped_d * clamped_d) / (sigma * sigma));
        // Inside the shape is always fully opaque
        aa = select(gauss, 1.0, d < 0.0);
    } else {
        // Crisp edges: smoothstep AA for sharp anti-aliased boundaries
        aa = 1.0 - smoothstep(-blur, blur, d);
    }

    // Multi-stop gradient: when gradient_config.x > 0.5, compute gradient color
    // from local_pos instead of using vertex-interpolated fill_color.
    // gradient_config = (stop_count, direction, mid_pos, 0)
    //   direction: 0.0 = horizontal (left->right), 1.0 = vertical (top->bottom)
    // Stops: fill_color = start/end color, gradient_color_mid = middle (symmetric).
    // Interpolation in linear space for sRGB-correct gradients (matching Loop #14).
    // Uses rotated `p` coordinate so gradient follows the quad's local axes.
    if (input.gradient_config.x > 0.5 && he.x > 0.0) {
        var t: f32;
        if (input.gradient_config.y > 0.5) {
            t = saturate((p.y + he.y) / (2.0 * he.y));
        } else {
            t = saturate((p.x + he.x) / (2.0 * he.x));
        }
        let mid_t = input.gradient_config.z;
        let start_lin = input.fill_color.rgb;
        let mid_lin = input.gradient_color_mid.rgb;
        var grad_lin: vec3<f32>;
        if (t < mid_t) {
            grad_lin = mix(start_lin, mid_lin, t / max(mid_t, 0.001));
        } else {
            grad_lin = mix(mid_lin, start_lin, (t - mid_t) / max(1.0 - mid_t, 0.001));
        }
        fill = vec4<f32>(linear_to_srgb(grad_lin), fill.a);
    }

    // Determine pixel color (fill or border)
    // Per-side border: pick the effective width based on which edge the fragment
    // is nearest to.  border_widths = (top, right, bottom, left) in CSS order.
    let bw = input.border_widths;
    let max_bw = max(bw.x, max(bw.y, max(bw.z, bw.w)));
    var color = fill;
    let has_border = max_bw > 0.0;
    if (has_border) {
        // Select per-side border width using miter diagonals (CSS box model):
        // the fragment belongs to whichever side's edge it is closest to.
        let nx = p.x / max(he.x, 0.001);  // -1..+1
        let ny = p.y / max(he.y, 0.001);  // -1..+1
        let ax = abs(nx);
        let ay = abs(ny);
        var eff_bw: f32;
        if (ay >= ax) {
            eff_bw = select(bw.z, bw.x, ny < 0.0);  // top or bottom
        } else {
            eff_bw = select(bw.w, bw.y, nx > 0.0);  // right or left
        }
        let inner_d = d + eff_bw;
        // For thin borders, clamp AA band so it doesn't exceed half the
        // border width — prevents the fill/border transition from consuming
        // more than its share of a sub-2px border at high DPI.
        let border_aa = min(crisp_aa, eff_bw * 0.5);
        let fill_mask = 1.0 - smoothstep(-border_aa, border_aa, inner_d);

        // Border 3D rim lighting: directional gradient across the border
        // using a top-left light source.  Gated by lighting_intensity so
        // flat CSS-like elements (lit=0) get perfectly uniform borders.
        let bny = p.y / max(he.y, 1.0);  // -1 top, +1 bottom
        let bnx = p.x / max(he.x, 1.0);  // -1 left, +1 right
        let border_highlight = 0.025 * saturate(-bny * 0.4 - bnx * 0.15 + 0.3) * input.lighting_intensity;
        let border_rgb = input.border_color.rgb + vec3<f32>(border_highlight);

        color = vec4<f32>(
            mix(border_rgb, fill.rgb, fill_mask),
            mix(input.border_color.a, fill.a, fill_mask),
        );
    }

    // Surface lighting model for filled SDF shapes.
    // Combines specular, environmental reflection, rim light, and ambient
    // occlusion to give UI elements a glass-like 3D quality reminiscent of
    // polished native app chrome (Zed, VS Code, macOS system UI).
    //
    // All terms use a consistent top-left light direction, matching the
    // directional panel cast shadows in the compositor (tab bar casts the
    // strongest shadow downward, sidebar casts rightward).
    //
    // Lighting extends smoothly to the fill edge (fading out near the border)
    // so there's no visible discontinuity where the lit fill meets unlit border.
    let edge_margin = select(1.5, max_bw + 0.5, has_border);
    let interior_t = saturate((-d - 0.5) / edge_margin);
    let lit = input.lighting_intensity;
    if (lit > 0.0 && interior_t > 0.0 && fill.a > 0.1 && blur <= 2.0 && !is_inner_shadow) {
        let ny = p.y / he.y;  // -1 at top, +1 at bottom
        let nx = p.x / he.x;  // -1 at left, +1 at right

        // Top-edge specular with horizontal falloff: bright near top-left,
        // fades toward bottom-right.  The x-axis contribution is subtle
        // (0.1 vs 0.2 offset) so the effect is primarily vertical but not
        // perfectly symmetric — matching a top-left light vector.
        let spec_t = saturate((-ny - 0.2) * 1.5);
        let spec_x = 1.0 - saturate(nx * 0.15);  // left side ~15% brighter
        let spec = spec_t * spec_t * 0.04 * spec_x;

        // Bottom-right darkening: counterpart to specular, creates "volume".
        // Slightly stronger at bottom-right (farthest from light source).
        let dark_t = saturate((ny - 0.3) * 1.2);
        let dark_x = 1.0 + saturate(nx * 0.2) * 0.3;  // right side ~30% darker
        let dark = dark_t * dark_t * 0.02 * dark_x;

        // Environmental reflection band: soft horizontal highlight at ~30%
        // from the top.  Simulates overhead ambient light reflecting off a
        // slightly convex surface — the "gel capsule" sheen seen on polished
        // native UI controls.  Gaussian profile for natural falloff.
        let refl_center = -0.3;
        let refl_dist = ny - refl_center;
        let refl = exp(-8.0 * refl_dist * refl_dist) * 0.008;

        // Edge-proximity rim light: Fresnel-like brightening near the SDF
        // boundary.  Strongest at the top-left (light source direction) and
        // fades toward the bottom-right for directional depth.
        let edge_dist = -d - select(0.5, max_bw, has_border);
        let rim_band = 2.5;  // width in pixels of the rim highlight zone
        let rim_raw = saturate(1.0 - edge_dist / rim_band);
        let dir_bias = saturate((-ny * 0.5 - nx * 0.25 + 0.4) * 0.8 + 0.2);
        let rim = rim_raw * rim_raw * dir_bias * 0.025;

        // Corner ambient occlusion: directionally weighted darkening where
        // two edges converge.  Bottom-right corner is darkest (farthest from
        // top-left light source), top-left is lightest.  This asymmetry
        // reinforces the directional lighting model used by panel shadows.
        let corner_x = saturate((abs(nx) - 0.4) * 2.5);
        let corner_y = saturate((abs(ny) - 0.4) * 2.5);
        let corner_base = corner_x * corner_y;
        // Directional weight: 1.0 at bottom-right, ~0.4 at top-left
        let corner_dir = 0.7 + 0.3 * saturate((nx + ny) * 0.5 + 0.5);
        let corner_ao = corner_base * corner_dir * 0.012;

        // Glass-edge highlight: concentrated bright line at the very top of
        // the element, mimicking the CSS `inset 0 1px 0 rgba(255,255,255,0.1)`
        // pattern.  Tighter than the specular term — a sharp Gaussian peak at
        // the topmost 2-3 pixels of the fill.  Creates the "wet edge" seen on
        // polished native controls (macOS buttons, Arc browser tabs).
        // A secondary, subtler peak at the left edge completes the top-left
        // light illusion.
        let top_edge = exp(-32.0 * (ny + 0.95) * (ny + 0.95)) * 0.035;
        let left_edge = exp(-32.0 * (nx + 0.95) * (nx + 0.95)) * 0.012;
        let edge_peak = top_edge + left_edge;

        let lighting = (spec + refl + rim + edge_peak - dark - corner_ao) * interior_t * lit;

        // Material micro-texture: spatially-coherent noise that gives the
        // surface a subtle "brushed matte" quality.  Without this, SDF shapes
        // are perfectly smooth digital rectangles; with it, they feel like
        // real material surfaces under ambient light.
        let grain = material_grain(screen_pos) * interior_t * lit;

        color = vec4<f32>(color.rgb + vec3<f32>(lighting + grain), color.a);
    }

    // Premultiplied alpha: RGB is pre-scaled by final alpha so the blend
    // unit can use (One, OneMinusSrcAlpha) — the industry standard for UI
    // compositing (Skia, Direct2D, CoreGraphics).
    let final_a = color.a * aa * clip_alpha;
    let linear = srgb_to_linear(color.rgb);
    let rgb = linear * final_a;
    return vec4<f32>(rgb, final_a);
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
        if vertices.is_empty() {
            return;
        }

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
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
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
        border_widths: [0.0; 4],
        border_color: [0.0, 0.0, 0.0, 0.0],
        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity: 0.0, // flat quads have no SDF lighting
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness: 0.0,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
    };

    [
        v([x0, y0]),
        v([x1, y0]),
        v([x0, y1]),
        v([x0, y1]),
        v([x1, y0]),
        v([x1, y1]),
    ]
}

/// Build 6 vertices for a flat gradient rectangle (no SDF).
/// Top vertices get `top_color`, bottom vertices get `bottom_color`.
pub fn quad_vertices_gradient(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
) -> [QuadVertex; 6] {
    quad_vertices_gradient_dir(
        x,
        y,
        w,
        h,
        viewport_w,
        viewport_h,
        top_color,
        bottom_color,
        false,
    )
}

/// Build 6 vertices for a flat horizontal gradient rectangle (no SDF).
/// Left vertices get `left_color`, right vertices get `right_color`.
pub fn quad_vertices_gradient_h(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    left_color: [f32; 4],
    right_color: [f32; 4],
) -> [QuadVertex; 6] {
    quad_vertices_gradient_dir(
        x,
        y,
        w,
        h,
        viewport_w,
        viewport_h,
        left_color,
        right_color,
        true,
    )
}

/// Internal: vertical or horizontal gradient.
fn quad_vertices_gradient_dir(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
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
        border_widths: [0.0; 4],
        border_color: [0.0, 0.0, 0.0, 0.0],
        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity: 0.0, // flat quads have no SDF lighting
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness: 0.0,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
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
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    fill_color: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    blur_radius: f32,
    lighting_intensity: f32,
    corner_smoothness: f32,
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

    // Expand geometry for AA (more expansion for outer blur/shadow).
    // Inner shadows (negative blur_radius) don't need expansion — they render inside.
    let pad = if blur_radius > 0.0 {
        blur_radius + 1.0
    } else {
        1.0
    };
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
        border_widths: [border_width; 4],
        border_color,
        blur_radius,
        rotation: 0.0,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
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

/// Like [`quad_vertices_sdf`] but accepts per-side border widths (top, right, bottom, left).
pub fn quad_vertices_sdf_bordered(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    fill_color: [f32; 4],
    corner_radii: [f32; 4],
    border_widths: [f32; 4],
    border_color: [f32; 4],
    blur_radius: f32,
    lighting_intensity: f32,
    corner_smoothness: f32,
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

    let pad = if blur_radius > 0.0 {
        blur_radius + 1.0
    } else {
        1.0
    };
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

    let v = |pos: [f32; 2], lp: [f32; 2]| QuadVertex {
        position: pos,
        fill_color,
        local_pos: lp,
        rect_half_ext: he,
        corner_radii: radii,
        border_widths,
        border_color,
        blur_radius,
        rotation: 0.0,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
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
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    fill_color_top: [f32; 4],
    fill_color_bottom: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    lighting_intensity: f32,
    corner_smoothness: f32,
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
        border_widths: [border_width; 4],
        border_color,
        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
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

/// Build 6 vertices for an SDF rounded rectangle with a horizontal gradient.
/// Left vertices get `fill_color_left`, right get `fill_color_right`.
/// The GPU interpolates the color smoothly across the shape.
pub fn quad_vertices_sdf_gradient_h(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    fill_color_left: [f32; 4],
    fill_color_right: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    lighting_intensity: f32,
    corner_smoothness: f32,
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
        border_widths: [border_width; 4],
        border_color,
        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
    };

    // Horizontal: left=fill_color_left, right=fill_color_right
    [
        v([x0, y0], [lx0, ly0], fill_color_left),
        v([x1, y0], [lx1, ly0], fill_color_right),
        v([x0, y1], [lx0, ly1], fill_color_left),
        v([x0, y1], [lx0, ly1], fill_color_left),
        v([x1, y0], [lx1, ly0], fill_color_right),
        v([x1, y1], [lx1, ly1], fill_color_right),
    ]
}

/// Build 6 vertices for an SDF rounded rectangle with a 3-stop symmetric gradient.
///
/// The gradient runs start -> mid -> start along the specified axis.
/// `direction`: 0.0 = horizontal (left->right), 1.0 = vertical (top->bottom).
/// `mid_pos`: position of the middle stop (0.0-1.0, typically 0.5).
/// All vertices share the same colors; the fragment shader computes the gradient.
pub fn quad_vertices_sdf_gradient_3stop(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    viewport_w: f32,
    viewport_h: f32,
    color_start: [f32; 4],
    color_mid: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    lighting_intensity: f32,
    corner_smoothness: f32,
    direction: f32,
    mid_pos: f32,
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

    let v = |pos: [f32; 2], lp: [f32; 2]| QuadVertex {
        position: pos,
        fill_color: color_start,
        local_pos: lp,
        rect_half_ext: he,
        corner_radii: radii,
        border_widths: [border_width; 4],
        border_color,
        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: color_mid,
        gradient_config: [3.0, direction, mid_pos, 0.0],
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

/// Build 6 vertices for a rotated SDF rounded rectangle.
///
/// `cx`, `cy` = center position in pixels.
/// `w`, `h` = unrotated rect dimensions in pixels.
/// `rotation` = rotation angle in radians (positive = clockwise in screen coords).
/// The geometry is expanded to the axis-aligned bounding box of the rotated shape.
pub fn quad_vertices_sdf_rotated(
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    rotation: f32,
    viewport_w: f32,
    viewport_h: f32,
    fill_color: [f32; 4],
    corner_radii: [f32; 4],
    border_width: f32,
    border_color: [f32; 4],
    blur_radius: f32,
    lighting_intensity: f32,
    corner_smoothness: f32,
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

    // AABB of rotated rect
    let cos_r = rotation.cos().abs();
    let sin_r = rotation.sin().abs();
    let aabb_hx = half_w * cos_r + half_h * sin_r;
    let aabb_hy = half_w * sin_r + half_h * cos_r;

    // Expand for AA
    let pad = if blur_radius > 0.0 {
        blur_radius + 1.0
    } else {
        1.0
    };
    let ex = cx - aabb_hx - pad;
    let ey = cy - aabb_hy - pad;
    let ew = (aabb_hx + pad) * 2.0;
    let eh = (aabb_hy + pad) * 2.0;

    let x0 = ex / viewport_w * 2.0 - 1.0;
    let y0 = -(ey / viewport_h * 2.0 - 1.0);
    let x1 = (ex + ew) / viewport_w * 2.0 - 1.0;
    let y1 = -((ey + eh) / viewport_h * 2.0 - 1.0);

    let lx0 = -(aabb_hx + pad);
    let ly0 = -(aabb_hy + pad);
    let lx1 = aabb_hx + pad;
    let ly1 = aabb_hy + pad;

    let he = [half_w, half_h]; // unrotated half-extents for SDF

    let v = |pos: [f32; 2], lp: [f32; 2]| QuadVertex {
        position: pos,
        fill_color,
        local_pos: lp,
        rect_half_ext: he,
        corner_radii: radii,
        border_widths: [border_width; 4],
        border_color,
        blur_radius,
        rotation,
        lighting_intensity,
        clip_rect: [0.0, 0.0, 99999.0, 99999.0],
        corner_smoothness,
        gradient_color_mid: [0.0; 4],
        gradient_config: [0.0; 4],
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
