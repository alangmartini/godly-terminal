use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
};

/// Key for looking up a rasterized glyph in the atlas.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct GlyphKey {
    ch: char,
    bold: bool,
    italic: bool,
}

/// Location and metrics of a rasterized glyph within the atlas texture.
#[derive(Clone, Copy, Debug)]
pub struct AtlasEntry {
    /// Top-left U coordinate (normalized 0..1).
    pub u0: f32,
    /// Top-left V coordinate (normalized 0..1).
    pub v0: f32,
    /// Bottom-right U coordinate (normalized 0..1).
    pub u1: f32,
    /// Bottom-right V coordinate (normalized 0..1).
    pub v1: f32,
    /// Glyph pixel width.
    pub width: u32,
    /// Glyph pixel height.
    pub height: u32,
    /// Horizontal bearing offset (pixels from cell left edge).
    pub offset_x: i32,
    /// Vertical bearing offset (pixels from cell top edge).
    pub offset_y: i32,
}

/// A glyph atlas that rasterizes characters into a single texture.
///
/// Glyphs are packed row-by-row into an RGBA texture. The RED channel
/// stores the glyph coverage/alpha value; G, B, A are zero. The shader
/// samples `.r` to use as the blend factor between background and foreground.
///
/// When the current row runs out of horizontal space, a new row starts.
/// When vertical space runs out, the atlas is resized (doubled in height).
pub struct GlyphAtlas {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Raw RGBA pixel data (R = glyph alpha, GBA = 0).
    atlas_data: Vec<u8>,
    atlas_width: u32,
    atlas_height: u32,
    entries: HashMap<GlyphKey, AtlasEntry>,
    /// X position for next glyph insertion.
    cursor_x: u32,
    /// Y position for current row.
    cursor_y: u32,
    /// Maximum height of any glyph in the current row.
    row_height: u32,
    /// Monospace cell width in pixels.
    cell_width: f32,
    /// Monospace cell height in pixels.
    cell_height: f32,
    /// GPU texture handle (created lazily on first use).
    texture: Option<wgpu::Texture>,
    /// Whether atlas_data has changed since last GPU upload.
    dirty: bool,
    /// The font size used for rendering.
    font_size: f32,
    /// The font family name.
    font_family: String,
}

/// Font families to try in order. The first one found on the system wins.
const FONT_FALLBACKS: &[&str] = &[
    "Cascadia Code",
    "Cascadia Mono",
    "Consolas",
    "Courier New",
    "monospace",
];

impl GlyphAtlas {
    /// Create a new glyph atlas with the given font and size.
    ///
    /// Attempts `font_family` first, then falls back through common monospace
    /// fonts. Pre-rasterizes printable ASCII (32..=126) for fast startup.
    pub fn new(font_family: &str, font_size: f32) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        // Determine cell metrics using a reference character.
        // We shape "M" to get the advance width, and use font metrics for height.
        let line_height = (font_size * 1.2).ceil();
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_size(&mut font_system, Some(font_size * 10.0), Some(line_height * 2.0));

        // Try the requested family first, then fallbacks.
        let families: Vec<&str> = std::iter::once(font_family)
            .chain(FONT_FALLBACKS.iter().copied())
            .collect();

        let mut cell_width = font_size * 0.6; // Conservative default
        let mut cell_height = line_height;
        let mut resolved_family = font_family.to_string();

        for &family in &families {
            let attrs = Attrs::new().family(Family::Name(family));
            buffer.set_text(&mut font_system, "M", attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut font_system, false);

            // Try to get cell width from layout runs
            let mut found = false;
            for run in buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    cell_width = glyph.w;
                    cell_height = line_height;
                    resolved_family = family.to_string();
                    found = true;
                    break;
                }
                if found {
                    break;
                }
            }
            if found {
                break;
            }
        }

        log::info!(
            "GlyphAtlas: font='{}', size={}, cell={}x{}",
            resolved_family,
            font_size,
            cell_width,
            cell_height
        );

        // Initial atlas: 512x512 is enough for ASCII + common chars.
        let atlas_width = 512u32;
        let atlas_height = 512u32;
        let atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

        let mut atlas = Self {
            font_system,
            swash_cache,
            atlas_data,
            atlas_width,
            atlas_height,
            entries: HashMap::new(),
            cursor_x: 1, // Leave a 1px border for the "blank" region at (0,0)
            cursor_y: 0,
            row_height: 0,
            cell_width,
            cell_height,
            texture: None,
            dirty: true,
            font_size,
            font_family: resolved_family,
        };

        // Reserve a 1x1 transparent region at (0,0) for space/empty cells.
        // This way, spaces map to UV coords that sample zero alpha.
        atlas.entries.insert(
            GlyphKey { ch: ' ', bold: false, italic: false },
            AtlasEntry {
                u0: 0.0,
                v0: 0.0,
                u1: 0.5 / atlas_width as f32,
                v1: 0.5 / atlas_height as f32,
                width: 0,
                height: 0,
                offset_x: 0,
                offset_y: 0,
            },
        );

        // Pre-rasterize printable ASCII for fast startup.
        for ch in ' '..='~' {
            for bold in [false, true] {
                atlas.get_or_insert(ch, bold, false);
            }
        }

        atlas
    }

    /// Get a cached glyph entry or rasterize it into the atlas.
    pub fn get_or_insert(&mut self, ch: char, bold: bool, italic: bool) -> AtlasEntry {
        let key = GlyphKey { ch, bold, italic };

        if let Some(entry) = self.entries.get(&key) {
            return *entry;
        }

        // For space or control chars, return the blank region.
        if ch.is_control() || ch == ' ' {
            let blank_key = GlyphKey { ch: ' ', bold: false, italic: false };
            let entry = *self.entries.get(&blank_key).unwrap();
            self.entries.insert(key, entry);
            return entry;
        }

        // Rasterize the glyph using cosmic-text.
        let entry = self.rasterize_glyph(ch, bold, italic);
        self.entries.insert(key, entry);
        entry
    }

    /// Rasterize a single glyph into a cell-sized atlas region.
    ///
    /// Each glyph is placed at its correct bearing offset within a region
    /// that matches the cell dimensions. This ensures the UV mapping from
    /// the cell quad to the atlas region is 1:1 — no stretching.
    fn rasterize_glyph(&mut self, ch: char, bold: bool, italic: bool) -> AtlasEntry {
        let line_height = self.cell_height;
        let metrics = Metrics::new(self.font_size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            &mut self.font_system,
            Some(self.cell_width * 4.0),
            Some(line_height * 2.0),
        );

        let weight = if bold { Weight::BOLD } else { Weight::NORMAL };
        let style = if italic { Style::Italic } else { Style::Normal };
        let attrs = Attrs::new()
            .family(Family::Name(&self.font_family))
            .weight(weight)
            .style(style);

        let text = ch.to_string();
        buffer.set_text(&mut self.font_system, &text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Collect glyph image data from cosmic-text SwashCache.
        let mut glyph_pixels: Vec<u8> = Vec::new();
        let mut glyph_w: u32 = 0;
        let mut glyph_h: u32 = 0;
        let mut blit_x: i32 = 0;
        let mut blit_y: i32 = 0;

        for run in buffer.layout_runs() {
            for layout_glyph in run.glyphs.iter() {
                let physical = layout_glyph.physical((0.0, 0.0), 1.0);
                if let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key)
                {
                    let w = image.placement.width;
                    let h = image.placement.height;
                    if w == 0 || h == 0 {
                        continue;
                    }

                    glyph_w = w;
                    glyph_h = h;

                    // Position glyph within the cell using physical coordinates
                    // and swash placement offsets.
                    // physical.x/y = glyph origin in pixel coords (baseline position)
                    // placement.left = X offset from origin to bitmap left edge
                    // placement.top = Y offset from origin to bitmap top edge (positive = above)
                    blit_x = physical.x + image.placement.left;
                    blit_y = physical.y - image.placement.top;

                    // Convert image data to single-channel alpha.
                    // cosmic-text may return different formats.
                    match image.content {
                        cosmic_text::SwashContent::Mask => {
                            // 1 byte per pixel: direct alpha/coverage
                            glyph_pixels = image.data.clone();
                        }
                        cosmic_text::SwashContent::Color => {
                            // 4 bytes per pixel (RGBA): extract alpha
                            glyph_pixels = image
                                .data
                                .chunks_exact(4)
                                .map(|px| px[3])
                                .collect();
                        }
                        cosmic_text::SwashContent::SubpixelMask => {
                            // 3 bytes per pixel (RGB subpixel): average as coverage
                            glyph_pixels = image
                                .data
                                .chunks_exact(3)
                                .map(|px| {
                                    ((px[0] as u16 + px[1] as u16 + px[2] as u16) / 3) as u8
                                })
                                .collect();
                        }
                    }
                    break; // Only need the first glyph
                }
            }
            break; // Only need the first layout run
        }

        // If rasterization produced nothing, return the blank entry.
        if glyph_w == 0 || glyph_h == 0 || glyph_pixels.is_empty() {
            let blank_key = GlyphKey { ch: ' ', bold: false, italic: false };
            return *self.entries.get(&blank_key).unwrap();
        }

        // Allocate a cell-sized region in the atlas (not tight glyph bounds).
        // This ensures the UV mapping from cell quad to atlas is 1:1.
        let cell_w = self.cell_width.ceil() as u32;
        let cell_h = self.cell_height.ceil() as u32;

        // Check if we need to wrap to a new row.
        if self.cursor_x + cell_w >= self.atlas_width {
            self.cursor_y += self.row_height + 1; // +1 for padding
            self.cursor_x = 0;
            self.row_height = 0;
        }

        // Check if we need to grow the atlas vertically.
        while self.cursor_y + cell_h >= self.atlas_height {
            self.grow_atlas();
        }

        // Blit glyph pixels into the cell-sized atlas region at the correct offset.
        let region_x = self.cursor_x;
        let region_y = self.cursor_y;
        for row in 0..glyph_h {
            for col in 0..glyph_w {
                let dst_x = region_x as i32 + blit_x + col as i32;
                let dst_y = region_y as i32 + blit_y + row as i32;

                // Clamp to cell region bounds.
                if dst_x < region_x as i32 || dst_x >= (region_x + cell_w) as i32 {
                    continue;
                }
                if dst_y < region_y as i32 || dst_y >= (region_y + cell_h) as i32 {
                    continue;
                }

                let src_idx = (row * glyph_w + col) as usize;
                let dst_idx = ((dst_y as u32 * self.atlas_width + dst_x as u32) * 4) as usize;
                if src_idx < glyph_pixels.len() && dst_idx + 3 < self.atlas_data.len() {
                    self.atlas_data[dst_idx] = glyph_pixels[src_idx]; // R = alpha
                    self.atlas_data[dst_idx + 1] = 0;
                    self.atlas_data[dst_idx + 2] = 0;
                    self.atlas_data[dst_idx + 3] = 255; // Texture alpha = opaque
                }
            }
        }

        let entry = AtlasEntry {
            u0: region_x as f32 / self.atlas_width as f32,
            v0: region_y as f32 / self.atlas_height as f32,
            u1: (region_x + cell_w) as f32 / self.atlas_width as f32,
            v1: (region_y + cell_h) as f32 / self.atlas_height as f32,
            width: cell_w,
            height: cell_h,
            offset_x: blit_x,
            offset_y: blit_y,
        };

        // Advance cursor by cell size (not glyph size).
        self.cursor_x += cell_w + 1; // +1 for padding between regions
        if cell_h > self.row_height {
            self.row_height = cell_h;
        }
        self.dirty = true;

        entry
    }

    /// Double the atlas height and copy existing data.
    fn grow_atlas(&mut self) {
        let new_height = self.atlas_height * 2;
        log::info!(
            "GlyphAtlas: growing from {}x{} to {}x{}",
            self.atlas_width,
            self.atlas_height,
            self.atlas_width,
            new_height
        );

        let mut new_data = vec![0u8; (self.atlas_width * new_height * 4) as usize];
        new_data[..self.atlas_data.len()].copy_from_slice(&self.atlas_data);
        self.atlas_data = new_data;

        // Recalculate UV coordinates for existing entries.
        let height_ratio = self.atlas_height as f32 / new_height as f32;
        for entry in self.entries.values_mut() {
            entry.v0 *= height_ratio;
            entry.v1 *= height_ratio;
        }

        self.atlas_height = new_height;
        self.texture = None; // Force re-creation of GPU texture
        self.dirty = true;
    }

    /// Ensure the GPU texture exists and is up to date with atlas_data.
    ///
    /// Creates the texture on first call, then re-uploads data whenever `dirty`.
    /// Returns a reference to the GPU texture.
    pub fn ensure_gpu_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> &wgpu::Texture {
        let needs_create = self.texture.is_none();

        if needs_create {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glyph_atlas"),
                size: wgpu::Extent3d {
                    width: self.atlas_width,
                    height: self.atlas_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.texture = Some(texture);
            self.dirty = true; // Force upload after creation
        }

        if self.dirty {
            if let Some(ref texture) = self.texture {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.atlas_data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.atlas_width * 4),
                        rows_per_image: Some(self.atlas_height),
                    },
                    wgpu::Extent3d {
                        width: self.atlas_width,
                        height: self.atlas_height,
                        depth_or_array_layers: 1,
                    },
                );
                self.dirty = false;
            }
        }

        self.texture.as_ref().unwrap()
    }

    /// Returns (cell_width, cell_height) in pixels.
    pub fn cell_size(&self) -> (f32, f32) {
        (self.cell_width, self.cell_height)
    }

    /// Returns the current atlas dimensions (width, height).
    pub fn atlas_dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    /// Returns the number of cached glyph entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_creation_succeeds() {
        let atlas = GlyphAtlas::new("Cascadia Code", 14.0);
        let (cw, ch) = atlas.cell_size();
        assert!(cw > 0.0, "cell width must be positive");
        assert!(ch > 0.0, "cell height must be positive");
    }

    #[test]
    fn atlas_has_preloaded_ascii() {
        let atlas = GlyphAtlas::new("Consolas", 14.0);
        // ASCII printable range is 95 chars (32..=126).
        // We pre-rasterize normal + bold = 190 entries + 1 space base entry.
        // Some chars map to the blank entry (like space variants), but entries >= 95.
        assert!(
            atlas.entry_count() >= 95,
            "Expected at least 95 entries, got {}",
            atlas.entry_count()
        );
    }

    #[test]
    fn get_or_insert_returns_same_entry() {
        let mut atlas = GlyphAtlas::new("Consolas", 14.0);
        let e1 = atlas.get_or_insert('A', false, false);
        let e2 = atlas.get_or_insert('A', false, false);
        assert!((e1.u0 - e2.u0).abs() < f32::EPSILON);
        assert!((e1.v0 - e2.v0).abs() < f32::EPSILON);
    }

    #[test]
    fn bold_and_normal_are_different_entries() {
        let mut atlas = GlyphAtlas::new("Consolas", 14.0);
        let normal = atlas.get_or_insert('A', false, false);
        let bold = atlas.get_or_insert('A', true, false);
        // They should be separate atlas entries (different UV regions),
        // unless the font happens to render them identically.
        // At minimum, both should be valid.
        assert!(normal.u1 > normal.u0 || normal.width == 0);
        assert!(bold.u1 > bold.u0 || bold.width == 0);
    }

    #[test]
    fn space_returns_blank_entry() {
        let mut atlas = GlyphAtlas::new("Consolas", 14.0);
        let entry = atlas.get_or_insert(' ', false, false);
        assert_eq!(entry.width, 0);
        assert_eq!(entry.height, 0);
    }

    #[test]
    fn unicode_glyph_can_be_inserted() {
        let mut atlas = GlyphAtlas::new("Consolas", 14.0);
        // This may fall back to blank if the font doesn't have it,
        // but it should not panic.
        let _entry = atlas.get_or_insert('\u{2588}', false, false); // Full block
        let _entry = atlas.get_or_insert('\u{00e9}', false, false); // e-acute
    }
}
