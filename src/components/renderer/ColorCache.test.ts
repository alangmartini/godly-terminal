import { describe, it, expect } from 'vitest';
import { ColorCache } from './ColorCache';

describe('ColorCache', () => {
  describe('parse', () => {
    it('parses #RRGGBB hex colors', () => {
      const cache = new ColorCache();
      // #ff0000 → 0xFF0000FF
      expect(cache.parse('#ff0000')).toBe(0xFF0000FF);
      // #00ff00 → 0x00FF00FF
      expect(cache.parse('#00ff00')).toBe(0x00FF00FF);
      // #0000ff → 0x0000FFFF
      expect(cache.parse('#0000ff')).toBe(0x0000FFFF);
    });

    it('parses #RGB shorthand colors', () => {
      const cache = new ColorCache();
      // #f00 → #ff0000 → 0xFF0000FF
      expect(cache.parse('#f00')).toBe(0xFF0000FF);
      // #0f0 → #00ff00 → 0x00FF00FF
      expect(cache.parse('#0f0')).toBe(0x00FF00FF);
    });

    it('parses rgb() format', () => {
      const cache = new ColorCache();
      expect(cache.parse('rgb(255, 0, 0)')).toBe(0xFF0000FF);
      expect(cache.parse('rgb(0,255,0)')).toBe(0x00FF00FF);
      expect(cache.parse('rgb( 0 , 0 , 255 )')).toBe(0x0000FFFF);
    });

    it('returns white for unrecognized formats', () => {
      const cache = new ColorCache();
      expect(cache.parse('invalid')).toBe(0xFFFFFFFF);
      expect(cache.parse('')).toBe(0xFFFFFFFF);
    });

    it('caches repeated lookups', () => {
      const cache = new ColorCache();
      const first = cache.parse('#1e1e1e');
      const second = cache.parse('#1e1e1e');
      expect(first).toBe(second);
      expect(first).toBe(0x1E1E1EFF);
    });

    it('parses theme-typical colors correctly', () => {
      const cache = new ColorCache();
      // Terminal background
      expect(cache.parse('#1e1e1e')).toBe(0x1E1E1EFF);
      // Terminal foreground
      expect(cache.parse('#cccccc')).toBe(0xCCCCCCFF);
      // Selection background
      expect(cache.parse('#264f78')).toBe(0x264F78FF);
    });
  });

  describe('dim', () => {
    it('reduces RGB channels by ~33%', () => {
      const cache = new ColorCache();
      const white = 0xFFFFFFFF;
      const dimmed = cache.dim(white);
      const r = (dimmed >>> 24) & 0xFF;
      const g = (dimmed >>> 16) & 0xFF;
      const b = (dimmed >>> 8) & 0xFF;
      const a = dimmed & 0xFF;
      // 255 * 0.67 ≈ 171
      expect(r).toBe(171);
      expect(g).toBe(171);
      expect(b).toBe(171);
      expect(a).toBe(0xFF); // alpha unchanged
    });

    it('preserves alpha channel', () => {
      const cache = new ColorCache();
      const dimmed = cache.dim(0xFF0000FF);
      expect(dimmed & 0xFF).toBe(0xFF);
    });

    it('dims black to black', () => {
      const cache = new ColorCache();
      expect(cache.dim(0x000000FF)).toBe(0x000000FF);
    });
  });

  describe('clear', () => {
    it('clears the cache', () => {
      const cache = new ColorCache();
      cache.parse('#ff0000');
      cache.clear();
      // Should still work after clear (re-parses)
      expect(cache.parse('#ff0000')).toBe(0xFF0000FF);
    });
  });
});
