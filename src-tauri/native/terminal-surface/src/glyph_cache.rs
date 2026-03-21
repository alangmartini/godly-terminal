use std::collections::HashMap;

/// Lookup key for cached glyphs.
///
/// Font size is quantized to quarter-pixel steps (size * 4 as u16) to avoid
/// separate cache entries for sub-pixel size differences that produce
/// identical rasterization results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub codepoint: char,
    /// Quantized font size: `(font_size * 4.0) as u16`.
    pub size_q4: u16,
    pub bold: bool,
    pub italic: bool,
}

impl GlyphKey {
    pub fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            codepoint: ch,
            size_q4: (font_size * 4.0) as u16,
            bold,
            italic,
        }
    }
}

/// A cached rasterized glyph (alpha mask + metrics).
pub struct CachedGlyph {
    pub alpha: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub advance: f32,
}

/// In-memory cache for rasterized glyph bitmaps.
///
/// Glyphs are stored by (codepoint, quantized size, bold, italic).
/// Call `invalidate()` when the font changes (e.g., user switches font family)
/// to clear all entries and bump the generation counter.
pub struct GlyphCache {
    entries: HashMap<GlyphKey, CachedGlyph>,
    generation: u64,
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            generation: 0,
        }
    }

    /// Look up a cached glyph by key.
    pub fn get(&self, key: &GlyphKey) -> Option<&CachedGlyph> {
        self.entries.get(key)
    }

    /// Insert a rasterized glyph into the cache.
    pub fn insert(&mut self, key: GlyphKey, glyph: CachedGlyph) {
        self.entries.insert(key, glyph);
    }

    /// Clear all entries and increment the generation counter.
    pub fn invalidate(&mut self) {
        self.entries.clear();
        self.generation += 1;
    }

    /// Current cache generation (incremented on each invalidate).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Number of cached glyphs.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glyph(alpha_val: u8) -> CachedGlyph {
        CachedGlyph {
            alpha: vec![alpha_val; 4],
            width: 2,
            height: 2,
            bearing_x: 0,
            bearing_y: 10,
            advance: 8.0,
        }
    }

    #[test]
    fn insert_and_get() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        cache.insert(key, make_glyph(200));

        let g = cache.get(&key).unwrap();
        assert_eq!(g.width, 2);
        assert_eq!(g.height, 2);
        assert_eq!(g.alpha, vec![200; 4]);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let cache = GlyphCache::new();
        let key = GlyphKey::new('Z', 14.0, false, false);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn invalidate_clears_and_increments_generation() {
        let mut cache = GlyphCache::new();
        assert_eq!(cache.generation(), 0);

        cache.insert(GlyphKey::new('A', 14.0, false, false), make_glyph(100));
        assert_eq!(cache.len(), 1);

        cache.invalidate();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.generation(), 1);
        assert!(cache.get(&GlyphKey::new('A', 14.0, false, false)).is_none());
    }

    #[test]
    fn different_keys_no_collision() {
        let mut cache = GlyphCache::new();
        let normal = GlyphKey::new('A', 14.0, false, false);
        let bold = GlyphKey::new('A', 14.0, true, false);
        let italic = GlyphKey::new('A', 14.0, false, true);
        let diff_size = GlyphKey::new('A', 16.0, false, false);

        cache.insert(normal, make_glyph(100));
        cache.insert(bold, make_glyph(150));
        cache.insert(italic, make_glyph(120));
        cache.insert(diff_size, make_glyph(130));

        assert_eq!(cache.len(), 4);
        assert_eq!(cache.get(&normal).unwrap().alpha[0], 100);
        assert_eq!(cache.get(&bold).unwrap().alpha[0], 150);
        assert_eq!(cache.get(&italic).unwrap().alpha[0], 120);
        assert_eq!(cache.get(&diff_size).unwrap().alpha[0], 130);
    }

    #[test]
    fn same_char_different_bold_stored_separately() {
        let mut cache = GlyphCache::new();
        let k1 = GlyphKey::new('X', 14.0, false, false);
        let k2 = GlyphKey::new('X', 14.0, true, false);

        cache.insert(k1, make_glyph(50));
        cache.insert(k2, make_glyph(200));

        assert_eq!(cache.get(&k1).unwrap().alpha[0], 50);
        assert_eq!(cache.get(&k2).unwrap().alpha[0], 200);
    }
}
