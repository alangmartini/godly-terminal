### Changed
- **DirectWrite ClearType pixel renderer enabled by default** — on Windows, the terminal now uses DirectWrite for glyph rasterization with ClearType subpixel RGB. Falls back to swash (grayscale) on non-Windows or if DirectWrite init fails. The pixel renderer is now the default rendering path (#841)
