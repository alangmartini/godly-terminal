import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { GlyphAtlas } from './GlyphAtlas';

// Mock OffscreenCanvas for jsdom environment
class MockOffscreenCanvas {
  width: number;
  height: number;
  constructor(w: number, h: number) {
    this.width = w;
    this.height = h;
  }
  getContext() {
    return {
      font: '',
      textBaseline: 'alphabetic',
      fillStyle: '',
      clearRect: vi.fn(),
      fillText: vi.fn(),
      measureText: () => ({ width: 8 }),
    };
  }
}

// Mock WebGL2RenderingContext with tracked call order
function createMockGL() {
  const calls: { method: string; args: unknown[] }[] = [];

  const gl = {
    TEXTURE_2D: 0x0DE1,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_MAG_FILTER: 0x2800,
    TEXTURE_WRAP_S: 0x2802,
    TEXTURE_WRAP_T: 0x2803,
    NEAREST: 0x2600,
    CLAMP_TO_EDGE: 0x812F,
    RGBA: 0x1908,
    UNSIGNED_BYTE: 0x1401,
    UNPACK_PREMULTIPLY_ALPHA_WEBGL: 0x9241,
    createTexture: vi.fn(() => ({})),
    bindTexture: vi.fn(),
    texParameteri: vi.fn(),
    texImage2D: vi.fn((...args: unknown[]) => {
      calls.push({ method: 'texImage2D', args });
    }),
    pixelStorei: vi.fn((...args: unknown[]) => {
      calls.push({ method: 'pixelStorei', args });
    }),
    deleteTexture: vi.fn(),
  } as unknown as WebGL2RenderingContext;

  return { gl, calls };
}

describe('GlyphAtlas', () => {
  let atlas: GlyphAtlas;
  let originalOffscreenCanvas: typeof globalThis.OffscreenCanvas;

  beforeEach(() => {
    originalOffscreenCanvas = globalThis.OffscreenCanvas;
    (globalThis as Record<string, unknown>).OffscreenCanvas = MockOffscreenCanvas;
    atlas = new GlyphAtlas('monospace', 13, 1);
  });

  afterEach(() => {
    (globalThis as Record<string, unknown>).OffscreenCanvas = originalOffscreenCanvas;
  });

  describe('uploadToGL', () => {
    // Bug: atlas was uploaded with UNPACK_PREMULTIPLY_ALPHA_WEBGL=false (default),
    // causing the browser to un-premultiply canvas data. The shader's max(r,g,b)
    // then always returned 1.0 for white text, destroying all font antialiasing.
    it('sets UNPACK_PREMULTIPLY_ALPHA_WEBGL=1 before texImage2D and resets after', () => {
      const { gl, calls } = createMockGL();

      atlas.getGlyph('A', false, false);
      atlas.uploadToGL(gl);

      const relevantCalls = calls.filter(
        c => c.method === 'pixelStorei' || c.method === 'texImage2D'
      );

      expect(relevantCalls).toHaveLength(3);
      expect(relevantCalls[0]).toEqual({ method: 'pixelStorei', args: [gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1] });
      expect(relevantCalls[1].method).toBe('texImage2D');
      expect(relevantCalls[2]).toEqual({ method: 'pixelStorei', args: [gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, 0] });
    });

    it('skips upload when atlas is not dirty', () => {
      const { gl, calls } = createMockGL();

      atlas.getGlyph('A', false, false);
      atlas.uploadToGL(gl);
      calls.length = 0;

      atlas.uploadToGL(gl);

      const texUploads = calls.filter(c => c.method === 'texImage2D');
      expect(texUploads).toHaveLength(0);
    });
  });

  describe('getGlyph', () => {
    it('returns same entry for identical character+style', () => {
      const a = atlas.getGlyph('A', false, false);
      const b = atlas.getGlyph('A', false, false);
      expect(a).toBe(b);
    });

    it('returns different entries for different styles', () => {
      const regular = atlas.getGlyph('A', false, false);
      const bold = atlas.getGlyph('A', true, false);
      const italic = atlas.getGlyph('A', false, true);
      expect(regular).not.toBe(bold);
      expect(regular).not.toBe(italic);
    });

    it('produces entries with positive dimensions', () => {
      const entry = atlas.getGlyph('M', false, false);
      expect(entry.w).toBeGreaterThan(0);
      expect(entry.h).toBeGreaterThan(0);
    });
  });

  describe('populateAscii', () => {
    it('pre-populates printable ASCII range', () => {
      atlas.populateAscii();
      // Spot-check characters exist and have dimensions
      const bang = atlas.getGlyph('!', false, false);
      const tilde = atlas.getGlyph('~', false, false);
      const boldA = atlas.getGlyph('A', true, false);
      expect(bang.w).toBeGreaterThan(0);
      expect(tilde.w).toBeGreaterThan(0);
      expect(boldA.w).toBeGreaterThan(0);
    });
  });
});
