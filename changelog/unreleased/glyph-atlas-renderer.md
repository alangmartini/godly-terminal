### Added
- **Glyph atlas rendering pipeline** — New CPU-side glyph rasterization with swash, glyph cache, and pixel buffer compositor replaces per-cell `fill_text()` canvas calls for dramatically faster and higher-quality terminal text rendering
- **Image widget integration** — Terminal panes now use pre-rendered pixel buffers via Iced's image widget instead of canvas text rendering
- **Font fallback** — System font fallback chain for characters not in the primary terminal font
