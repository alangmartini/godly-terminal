### Added
- **Configurable text shader parameters** — text rendering gamma, enhanced contrast, luminance threshold, dark-bg gamma, and coverage attenuation are now runtime-configurable via GPU uniform buffer instead of hardcoded WGSL constants

### Changed
- **Text glyph weight reduced to match browser rendering** — coverage attenuation set to 0.92 to compensate for DirectWrite NATURAL_SYMMETRIC producing heavier stems than browser (Skia/HarfBuzz) rasterization
