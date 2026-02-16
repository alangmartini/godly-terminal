# WebGL Glyph Antialiasing Bug

**Status:** Resolved
**Regression Risk:** Medium — any change to GlyphAtlas texture upload or shader alpha logic

## Symptom

Text rendered in the WebGL renderer appears aliased/jagged with no font antialiasing. Characters look like bitmap pixel art instead of smooth ClearType/greyscale-antialiased text.

## Root Cause

The glyph atlas is rasterized on an OffscreenCanvas with white text on a transparent background. The canvas stores pixel data in **premultiplied alpha** format internally. For an edge pixel at 50% coverage, the internal representation is `RGBA = (128, 128, 128, 128)`.

When uploading via `texImage2D()`, WebGL's default `UNPACK_PREMULTIPLY_ALPHA_WEBGL = false` causes the browser to **un-premultiply** the data before upload, producing `RGBA = (255, 255, 255, 128)` → normalized `(1.0, 1.0, 1.0, 0.5)`.

The fragment shader recovers alpha via `max(texel.r, texel.g, texel.b)`. On un-premultiplied white text, RGB is always `1.0` regardless of actual coverage, so **every non-transparent pixel gets alpha=1.0**, completely destroying antialiasing.

## Fix

Set `gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1)` before the atlas `texImage2D` call to preserve premultiplied values. The shader's `max(r,g,b)` then correctly recovers the coverage alpha:

- 50% coverage pixel: texture `(0.5, 0.5, 0.5, 0.5)` → `max(r,g,b) = 0.5` ✓
- 100% coverage pixel: texture `(1.0, 1.0, 1.0, 1.0)` → `max(r,g,b) = 1.0` ✓
- Transparent pixel: texture `(0, 0, 0, 0)` → `max(r,g,b) = 0.0` ✓

## Files Changed

- `src/components/renderer/GlyphAtlas.ts` — Added `pixelStorei(UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1)` around atlas upload
- `src/components/renderer/GlyphAtlas.test.ts` — Tests verifying premultiplied alpha upload ordering
