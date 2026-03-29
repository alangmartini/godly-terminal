//! Builds per-cell vertex data for the GPU glyph atlas renderer.

use godly_protocol::types::{CursorShape, RichGridData};
use crate::Color;

use crate::colors::{brighten_color, dim_color, parse_color};
use crate::font_metrics::FontMetrics;
use crate::glyph_atlas::GlyphAtlas;
use crate::glyph_cache::GlyphKey;
use crate::glyph_rasterizer::GlyphRasterizer;

/// Per-vertex data uploaded to the GPU.  40 bytes per vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
}

impl CellVertex {
    /// wgpu vertex buffer layout descriptor.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // position
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            // uv
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            // fg_color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 2,
            },
            // bg_color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 3,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// Build the vertex buffer for one terminal frame.
pub fn build_vertices(
    grid: &RichGridData,
    atlas: &mut GlyphAtlas,
    metrics: &FontMetrics,
    rasterizer: &mut dyn GlyphRasterizer,
    default_fg: Color,
    default_bg: Color,
    viewport_w: u32,
    viewport_h: u32,
) -> Vec<CellVertex> {
    let phys = metrics.scaled_for_render();
    let cell_w = phys.cell_width;
    let cell_h = phys.cell_height;
    let font_size = phys.font_size;
    let vw = viewport_w as f32;
    let vh = viewport_h as f32;

    let cols = grid.dimensions.cols as usize;
    let rows = grid.dimensions.rows as usize;

    let mut verts = Vec::with_capacity(rows * cols * 6 + 64);

    for (row_idx, row) in grid.rows.iter().enumerate() {
        for (col_idx, cell) in row.cells.iter().enumerate() {
            if cell.wide_continuation {
                continue;
            }

            // --- resolve colours ---
            let mut fg = parse_color(&cell.fg, default_fg);
            let mut bg = parse_color(&cell.bg, default_bg);

            if cell.inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.dim {
                fg = dim_color(fg);
            }
            if cell.bold && fg != default_fg {
                fg = brighten_color(fg);
            }

            // --- pixel position (physical) ---
            let px = (col_idx as f32 * cell_w).round();
            let py = (row_idx as f32 * cell_h).round();
            let pw = if cell.wide {
                ((col_idx + 2) as f32 * cell_w).round() - px
            } else {
                ((col_idx + 1) as f32 * cell_w).round() - px
            };
            let ph = ((row_idx + 1) as f32 * cell_h).round() - py;

            // --- clip-space position ---
            let x0 = px / vw * 2.0 - 1.0;
            let y0 = 1.0 - py / vh * 2.0;
            let x1 = (px + pw) / vw * 2.0 - 1.0;
            let y1 = 1.0 - (py + ph) / vh * 2.0;

            // --- atlas UV ---
            let ch = cell.content.chars().next().unwrap_or(' ');
            let key = GlyphKey::new(ch, font_size, cell.bold, cell.italic);
            let entry = atlas.get_or_insert(key, rasterizer, font_size);

            let fg4 = [fg.r, fg.g, fg.b, fg.a];
            let bg4 = [bg.r, bg.g, bg.b, bg.a];

            // two triangles (6 verts): TL, TR, BL — TR, BL, BR
            push_quad(
                &mut verts,
                [x0, y0],
                [x1, y1],
                [entry.u0, entry.v0],
                [entry.u1, entry.v1],
                fg4,
                bg4,
            );

            // --- underline ---
            if cell.underline {
                let ul_y = py + ph - 2.0;
                let ul_y0 = 1.0 - ul_y / vh * 2.0;
                let ul_y1 = 1.0 - (ul_y + 1.0) / vh * 2.0;
                // Use a fully-opaque quad (atlas blank region alpha=0 → mix returns bg)
                // So we use fg as both fg and bg to get a solid-color line.
                push_quad(
                    &mut verts,
                    [x0, ul_y0],
                    [x1, ul_y1],
                    [0.0, 0.0],
                    [0.0, 0.0], // blank atlas region
                    fg4,
                    fg4, // solid fg colour
                );
            }
        }
    }

    // --- cursor ---
    if !grid.cursor_hidden {
        let cr = grid.cursor.row as usize;
        let cc = grid.cursor.col as usize;
        let cpx = (cc as f32 * cell_w).round();
        let cpy = (cr as f32 * cell_h).round();

        let cursor_color = [1.0f32, 1.0, 1.0, 0.8];

        match grid.cursor.cursor_style {
            CursorShape::BlinkBlock | CursorShape::SteadyBlock => {
                let cx0 = cpx / vw * 2.0 - 1.0;
                let cy0 = 1.0 - cpy / vh * 2.0;
                let cx1 = (cpx + cell_w) / vw * 2.0 - 1.0;
                let cy1 = 1.0 - (cpy + cell_h) / vh * 2.0;
                push_quad(
                    &mut verts,
                    [cx0, cy0],
                    [cx1, cy1],
                    [0.0, 0.0],
                    [0.0, 0.0],
                    cursor_color,
                    cursor_color,
                );
            }
            CursorShape::BlinkUnderline | CursorShape::SteadyUnderline => {
                let uy = cpy + cell_h - 2.0;
                let cx0 = cpx / vw * 2.0 - 1.0;
                let cy0 = 1.0 - uy / vh * 2.0;
                let cx1 = (cpx + cell_w) / vw * 2.0 - 1.0;
                let cy1 = 1.0 - (uy + 2.0) / vh * 2.0;
                push_quad(
                    &mut verts,
                    [cx0, cy0],
                    [cx1, cy1],
                    [0.0, 0.0],
                    [0.0, 0.0],
                    cursor_color,
                    cursor_color,
                );
            }
            CursorShape::BlinkBar | CursorShape::SteadyBar => {
                let cx0 = cpx / vw * 2.0 - 1.0;
                let cy0 = 1.0 - cpy / vh * 2.0;
                let cx1 = (cpx + 2.0) / vw * 2.0 - 1.0;
                let cy1 = 1.0 - (cpy + cell_h) / vh * 2.0;
                push_quad(
                    &mut verts,
                    [cx0, cy0],
                    [cx1, cy1],
                    [0.0, 0.0],
                    [0.0, 0.0],
                    cursor_color,
                    cursor_color,
                );
            }
        }
    }

    verts
}

/// Emit 6 vertices for a quad (two triangles).
fn push_quad(
    out: &mut Vec<CellVertex>,
    tl: [f32; 2],
    br: [f32; 2],
    uv_tl: [f32; 2],
    uv_br: [f32; 2],
    fg: [f32; 4],
    bg: [f32; 4],
) {
    let tr = [br[0], tl[1]];
    let bl = [tl[0], br[1]];
    let uv_tr = [uv_br[0], uv_tl[1]];
    let uv_bl = [uv_tl[0], uv_br[1]];

    let v = |pos: [f32; 2], uv: [f32; 2]| CellVertex {
        position: pos,
        uv,
        fg_color: fg,
        bg_color: bg,
    };

    // Triangle 1: TL, TR, BL
    out.push(v(tl, uv_tl));
    out.push(v(tr, uv_tr));
    out.push(v(bl, uv_bl));
    // Triangle 2: TR, BL, BR  (consistent winding)
    out.push(v(tr, uv_tr));
    out.push(v(bl, uv_bl));
    out.push(v(br, uv_br));
}
