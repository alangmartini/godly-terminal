### Fixed
- **DPI scale factor in text rendering** — thread the OS DPI scale factor through to DirectWrite and the pixel renderer so glyphs are rasterized at the correct physical resolution on HiDPI displays, and invalidate the glyph cache when the scale factor changes
