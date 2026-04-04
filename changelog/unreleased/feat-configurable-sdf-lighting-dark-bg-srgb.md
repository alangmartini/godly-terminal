### Fixed
- **Dark-background text compositing uses sRGB gamma** — Text on dark backgrounds now blends with sRGB gamma (2.2) matching browser compositing, instead of ClearType gamma (1.8) which produced slightly thicker text weight. Light backgrounds retain ClearType subpixel blending unchanged.
