### Fixed
- **DPI double-scaling in DirectWrite renderer** — Glyphs were rasterized at `font_size * scale_factor²` because the pixel renderer already passes physical-pixel font sizes but `CreateGlyphRunAnalysis` applied `pixelsPerDip` again. Fixed by using `pixelsPerDip = 1.0`.
- **Pixel buffer displayed at wrong size** — The physical-pixel image buffer was displayed with `ContentFit::None`, causing iced to treat physical dimensions as logical pixels (1.5× too large at 150% DPI). Changed to `ContentFit::Fill` for correct physical-to-logical mapping.
