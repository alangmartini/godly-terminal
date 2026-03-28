### Added

- **GPU glyph atlas renderer** — Replaces CPU pixel compositor with per-cell GPU quad rendering from a persistent glyph texture atlas. Glyphs are rasterized once and packed into a shared atlas texture; each terminal cell is drawn as a positioned textured quad. Eliminates ClearType composition artifacts (sub-pixel patterns don't survive offscreen GPU compositing) by using grayscale antialiasing. (#861)
