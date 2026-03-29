### Fixed

- **Subpixel text rendering quality** — Preserve DirectWrite ClearType RGB coverage through the GPU glyph atlas pipeline instead of averaging to grayscale, and blend in linear color space for gamma-correct antialiasing. Text now renders with proper weight and crispness matching native Windows text quality. (#863)
