### Fixed
- **Italic glyph clipping in glyph atlas** — glyphs with negative bearing_x (common in italic fonts like Georgia Italic) now render correctly instead of having their left edge clipped, fixing corrupted characters (e.g., 'v' appearing as a comma)
