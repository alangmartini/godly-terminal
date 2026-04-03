### Changed

- **Improved thin border anti-aliasing** — Clamped the SDF AA band to half the border width, preventing the fill/border transition from consuming too much of sub-2px borders at high DPI. Border 3D rim lighting now respects the `lighting_intensity` parameter so flat CSS-like elements get perfectly uniform borders.
