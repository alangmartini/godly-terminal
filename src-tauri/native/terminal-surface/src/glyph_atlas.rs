//! GPU glyph atlas — packs rasterized glyphs into a persistent texture.
//!
//! Glyphs are rasterized on the CPU (via DirectWrite / swash) and packed
//! row-by-row into an RGBA buffer.  For ClearType subpixel glyphs, the
//! **R, G, B channels** store per-subpixel alpha coverage directly,
//! enabling LCD-quality text rendering (3× horizontal effective resolution).
//! For grayscale glyphs (e.g. from Swash), the same alpha value is
//! replicated to all three channels.  The A channel is always 0xFF for
//! occupied texels.

use std::collections::HashMap;

use crate::glyph_cache::GlyphKey;
use crate::glyph_rasterizer::{GlyphFormat, GlyphRasterizer};

fn glyph_origin_y(baseline_offset: u32, bearing_y: i32, clamp_top: bool) -> i32 {
    let y = baseline_offset as i32 - bearing_y;
    if clamp_top { y.max(0) } else { y }
}

/// Position of a glyph inside the atlas (normalised UV coordinates).
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Actual horizontal advance width in pixels (for proportional font positioning).
    pub advance: f32,
}

/// Data bundle for uploading the atlas to the GPU.
#[derive(Debug, Clone)]
pub struct AtlasUpdate {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub generation: u64,
}

/// CPU-side glyph atlas with row-by-row packing.
pub struct GlyphAtlas {
    entries: HashMap<GlyphKey, AtlasEntry>,
    /// RGBA pixel data.  R, G, B channels store per-subpixel coverage
    /// (for ClearType) or identical alpha (for grayscale).  A = 0xFF for
    /// occupied texels, 0x00 for empty.
    data: Vec<u8>,
    width: u32,
    height: u32,
    /// Next free X position in the current row.
    cursor_x: u32,
    /// Y position of the current row.
    cursor_y: u32,
    /// Height of the tallest glyph in the current row.
    row_height: u32,
    /// Monotonically increasing; bumped on resize / invalidate.
    generation: u64,
    dirty: bool,
    /// Cell dimensions in physical pixels (set once at init / font change).
    cell_w: u32,
    cell_h: u32,
    /// Baseline offset within a cell (physical pixels).
    baseline_offset: u32,
}

const INITIAL_SIZE: u32 = 1024;
const PADDING: u32 = 1;

impl GlyphAtlas {
    /// Create a new atlas sized for the given cell metrics.
    ///
    /// Call `precache_ascii` after creation to warm the cache.
    pub fn new(cell_w: f32, cell_h: f32, baseline_offset: f32) -> Self {
        let cw = cell_w.ceil() as u32;
        let ch = cell_h.ceil() as u32;
        let w = INITIAL_SIZE;
        let h = INITIAL_SIZE;
        Self {
            entries: HashMap::new(),
            data: vec![0u8; (w * h * 4) as usize],
            width: w,
            height: h,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            generation: 1,
            dirty: true,
            cell_w: cw,
            cell_h: ch,
            baseline_offset: baseline_offset.round() as u32,
        }
    }

    /// Pre-cache printable ASCII (32–126) in normal + bold variants.
    pub fn precache_ascii(&mut self, rasterizer: &mut dyn GlyphRasterizer, font_size_px: f32) {
        for ch in ' '..='~' {
            for bold in [false, true] {
                let key = GlyphKey::new(ch, font_size_px, bold, false);
                self.get_or_insert(key, rasterizer, font_size_px);
            }
        }
    }

    /// Look up a glyph, rasterizing and packing it on cache miss.
    ///
    /// Returns `None` only if the rasterizer fails (e.g. missing glyph).
    /// For missing glyphs a blank cell-sized entry is returned so the
    /// vertex builder always has valid UVs.
    pub fn get_or_insert(
        &mut self,
        key: GlyphKey,
        rasterizer: &mut dyn GlyphRasterizer,
        font_size_px: f32,
    ) -> AtlasEntry {
        if let Some(e) = self.entries.get(&key) {
            return *e;
        }

        // Rasterize the glyph.
        let glyph = rasterizer.rasterize(key.codepoint, font_size_px, key.weight, key.italic);

        // For UI font glyphs, use actual glyph width for slot sizing (proportional).
        // For terminal font, use the fixed cell_w.
        let slot_w = if key.font_id != 0 {
            if let Some(g) = &glyph {
                // Slot must fit the glyph bitmap (bearing_x + width) and be at least advance-wide
                let bitmap_extent = (g.bearing_x.max(0) as u32).saturating_add(g.width);
                let advance_w = g.advance.ceil() as u32;
                bitmap_extent.max(advance_w).max(1)
            } else {
                self.cell_w
            }
        } else {
            self.cell_w
        };

        let advance = glyph.as_ref().map_or(self.cell_w as f32, |g| g.advance);

        // Convert to RGBA subpixel coverage and blit into a slot-sized region.
        let rgba = glyph.as_ref().map(|g| to_rgba_coverage(g));
        let entry = self.pack_slot(
            rgba.as_ref(),
            glyph.as_ref(),
            slot_w,
            advance,
            key.font_id != 0,
        );
        self.entries.insert(key, entry);
        entry
    }

    /// Pack a glyph bitmap (RGBA subpixel coverage) into the next available
    /// slot in the atlas. `slot_w` controls horizontal allocation size
    /// (cell_w for monospace, actual glyph width for proportional).
    fn pack_slot(
        &mut self,
        rgba: Option<&Vec<u8>>,
        glyph: Option<&crate::glyph_rasterizer::RasterizedGlyph>,
        slot_w: u32,
        advance: f32,
        clamp_top_for_ui_fonts: bool,
    ) -> AtlasEntry {
        let sw = slot_w;
        let ch = self.cell_h;

        // Wrap to next row if current row is full.
        if self.cursor_x + sw > self.width {
            self.cursor_y += self.row_height + PADDING;
            self.cursor_x = 0;
            self.row_height = 0;
        }

        // Grow atlas vertically if needed.
        while self.cursor_y + ch > self.height {
            self.grow();
        }

        let x0 = self.cursor_x;
        let y0 = self.cursor_y;

        // Blit the glyph bitmap at the correct bearing offset within the slot.
        if let (Some(rgba_data), Some(g)) = (rgba, glyph) {
            if g.width > 0 && g.height > 0 {
                // Glyph origin within the slot:
                let gx = g.bearing_x;
                let gy = glyph_origin_y(self.baseline_offset, g.bearing_y, clamp_top_for_ui_fonts);
                for row in 0..g.height {
                    for col in 0..g.width {
                        let dst_x = x0 as i32 + gx + col as i32;
                        let dst_y = y0 as i32 + gy + row as i32;
                        if dst_x >= 0
                            && dst_y >= 0
                            && (dst_x as u32) < self.width
                            && (dst_y as u32) < self.height
                            && (dst_x as u32) < x0 + sw
                        // clip to slot boundary
                        {
                            let src_idx = (row * g.width + col) as usize * 4;
                            let dst_idx = ((dst_y as u32) * self.width + dst_x as u32) as usize * 4;
                            if src_idx + 3 < rgba_data.len() && dst_idx + 3 < self.data.len() {
                                self.data[dst_idx] = rgba_data[src_idx]; // R
                                self.data[dst_idx + 1] = rgba_data[src_idx + 1]; // G
                                self.data[dst_idx + 2] = rgba_data[src_idx + 2]; // B
                                self.data[dst_idx + 3] = rgba_data[src_idx + 3];
                                // A
                            }
                        }
                    }
                }
            }
        }

        // Update packing cursor.
        self.cursor_x += sw + PADDING;
        if ch > self.row_height {
            self.row_height = ch;
        }
        self.dirty = true;

        // Compute normalised UV coordinates.
        let u0 = x0 as f32 / self.width as f32;
        let v0 = y0 as f32 / self.height as f32;
        let u1 = (x0 + sw) as f32 / self.width as f32;
        let v1 = (y0 + ch) as f32 / self.height as f32;

        AtlasEntry {
            u0,
            v0,
            u1,
            v1,
            advance,
        }
    }

    /// Double the atlas height, preserving existing data and rescaling UVs.
    fn grow(&mut self) {
        let old_h = self.height;
        let new_h = old_h * 2;
        let mut new_data = vec![0u8; (self.width * new_h * 4) as usize];
        let row_bytes = (self.width * 4) as usize;
        for y in 0..old_h {
            let src = (y as usize) * row_bytes;
            new_data[src..src + row_bytes].copy_from_slice(&self.data[src..src + row_bytes]);
        }
        self.data = new_data;
        let scale = old_h as f32 / new_h as f32;
        for entry in self.entries.values_mut() {
            entry.v0 *= scale;
            entry.v1 *= scale;
        }
        self.height = new_h;
        self.generation += 1;
        self.dirty = true;
    }

    /// Current atlas texture width in pixels.
    pub fn atlas_width(&self) -> u32 {
        self.width
    }

    /// Clear the atlas (e.g. on font change). Bumps generation.
    pub fn invalidate(&mut self) {
        self.entries.clear();
        self.data.fill(0);
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
        self.generation += 1;
        self.dirty = true;
    }

    /// Update cell metrics (e.g. after font size change).
    pub fn set_cell_metrics(&mut self, cell_w: f32, cell_h: f32, baseline_offset: f32) {
        let cw = cell_w.ceil() as u32;
        let ch = cell_h.ceil() as u32;
        if cw != self.cell_w || ch != self.cell_h {
            self.cell_w = cw;
            self.cell_h = ch;
            self.baseline_offset = baseline_offset.round() as u32;
            self.invalidate();
        }
    }

    /// Take dirty data for GPU upload.  Returns `None` if the atlas hasn't
    /// changed since the last call.
    pub fn take_dirty_data(&mut self) -> Option<AtlasUpdate> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(AtlasUpdate {
            data: self.data.clone(),
            width: self.width,
            height: self.height,
            generation: self.generation,
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::glyph_origin_y;

    #[test]
    fn ui_glyph_origin_clamps_negative_top_offsets() {
        assert_eq!(glyph_origin_y(11, 13, true), 0);
        assert_eq!(glyph_origin_y(11, 13, false), -2);
    }
}

/// Convert a rasterized glyph to RGBA subpixel coverage.
///
/// For ClearType subpixel glyphs, R/G/B channels carry per-subpixel
/// alpha and A = max(R,G,B) so background shows through uniformly.
/// For grayscale glyphs, the alpha value is replicated to all channels.
fn to_rgba_coverage(g: &crate::glyph_rasterizer::RasterizedGlyph) -> Vec<u8> {
    let pixel_count = (g.width * g.height) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    match g.format {
        GlyphFormat::Alpha => {
            for i in 0..pixel_count {
                let a = g.data[i];
                rgba.push(a); // R
                rgba.push(a); // G
                rgba.push(a); // B
                rgba.push(a); // A
            }
        }
        GlyphFormat::SubpixelRgb => {
            for i in 0..pixel_count {
                let r = g.data[i * 3];
                let g_ch = g.data[i * 3 + 1];
                let b = g.data[i * 3 + 2];
                rgba.push(r);
                rgba.push(g_ch);
                rgba.push(b);
                // A = max coverage so compositor knows this texel is occupied
                rgba.push(r.max(g_ch).max(b));
            }
        }
    }
    rgba
}
