use std::collections::HashMap;

use crate::glyph_rasterizer::GlyphFormat;

/// Snapshot of cache hit/miss counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

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
    /// Font identifier: 0 = terminal monospace, 1 = UI proportional.
    pub font_id: u8,
}

impl GlyphKey {
    pub fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            codepoint: ch,
            size_q4: (font_size * 4.0) as u16,
            bold,
            italic,
            font_id: 0,
        }
    }

    /// Create a key for UI font glyphs (proportional sans-serif).
    pub fn new_ui(ch: char, font_size: f32, bold: bool) -> Self {
        Self {
            codepoint: ch,
            size_q4: (font_size * 4.0) as u16,
            bold,
            italic: false,
            font_id: 1,
        }
    }
}

/// A cached rasterized glyph (bitmap data + metrics).
pub struct CachedGlyph {
    /// Glyph bitmap data. Layout depends on `format`:
    /// - `Alpha`: 1 byte per pixel (8-bit coverage).
    /// - `SubpixelRgb`: 3 bytes per pixel (R, G, B coverage values).
    pub data: Vec<u8>,
    /// Format of the data stored in `data`.
    pub format: GlyphFormat,
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
    hits: u64,
    misses: u64,
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            generation: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a cached glyph by key, tracking hits and misses.
    pub fn get(&mut self, key: &GlyphKey) -> Option<&CachedGlyph> {
        if self.entries.contains_key(key) {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        self.entries.get(key)
    }

    /// Insert a rasterized glyph into the cache.
    pub fn insert(&mut self, key: GlyphKey, glyph: CachedGlyph) {
        self.entries.insert(key, glyph);
    }

    /// Clear all entries, increment the generation counter, and reset stats.
    pub fn invalidate(&mut self) {
        self.entries.clear();
        self.generation += 1;
        self.hits = 0;
        self.misses = 0;
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

    /// Returns a snapshot of the current hit/miss counters.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
        }
    }

    /// Reset hit/miss counters to zero.
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glyph(data_val: u8) -> CachedGlyph {
        CachedGlyph {
            data: vec![data_val; 4],
            format: GlyphFormat::Alpha,
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
        assert_eq!(g.data, vec![200; 4]);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let mut cache = GlyphCache::new();
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
        assert_eq!(cache.get(&normal).unwrap().data[0], 100);
        assert_eq!(cache.get(&bold).unwrap().data[0], 150);
        assert_eq!(cache.get(&italic).unwrap().data[0], 120);
        assert_eq!(cache.get(&diff_size).unwrap().data[0], 130);
    }

    #[test]
    fn same_char_different_bold_stored_separately() {
        let mut cache = GlyphCache::new();
        let k1 = GlyphKey::new('X', 14.0, false, false);
        let k2 = GlyphKey::new('X', 14.0, true, false);

        cache.insert(k1, make_glyph(50));
        cache.insert(k2, make_glyph(200));

        assert_eq!(cache.get(&k1).unwrap().data[0], 50);
        assert_eq!(cache.get(&k2).unwrap().data[0], 200);
    }

    #[test]
    fn hit_increments_on_found() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        cache.insert(key, make_glyph(100));

        let _ = cache.get(&key);
        let s = cache.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn miss_increments_on_not_found() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('Z', 14.0, false, false);

        let _ = cache.get(&key);
        let s = cache.stats();
        assert_eq!(s.hits, 0);
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn mixed_hit_miss_tracking() {
        let mut cache = GlyphCache::new();
        let present = GlyphKey::new('A', 14.0, false, false);
        let absent = GlyphKey::new('Z', 14.0, false, false);
        cache.insert(present, make_glyph(100));

        let _ = cache.get(&present);
        let _ = cache.get(&absent);
        let s = cache.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn reset_stats_clears_counters() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        cache.insert(key, make_glyph(100));

        let _ = cache.get(&key);
        let _ = cache.get(&GlyphKey::new('Z', 14.0, false, false));
        cache.reset_stats();

        let s = cache.stats();
        assert_eq!(s.hits, 0);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn invalidate_resets_stats() {
        let mut cache = GlyphCache::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        cache.insert(key, make_glyph(100));

        let _ = cache.get(&key);
        let _ = cache.get(&GlyphKey::new('Z', 14.0, false, false));
        cache.invalidate();

        let s = cache.stats();
        assert_eq!(s.hits, 0);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn hit_rate_calculation() {
        let s = CacheStats::default();
        assert_eq!(s.hit_rate(), 0.0);

        let s = CacheStats { hits: 1, misses: 1 };
        assert!((s.hit_rate() - 0.5).abs() < f64::EPSILON);

        let s = CacheStats {
            hits: 10,
            misses: 0,
        };
        assert!((s.hit_rate() - 1.0).abs() < f64::EPSILON);
    }
}
