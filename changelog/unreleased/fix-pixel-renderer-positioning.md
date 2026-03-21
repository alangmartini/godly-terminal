### Fixed
- **Pixel renderer fractional positioning** — Fixed character misalignment and screen trembling by using fractional cell positions instead of integer `ceil()` rounding, matching the canvas renderer's coordinate system
